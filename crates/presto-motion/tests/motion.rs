//! presto-motion's own behaviour: the OP_ANIMATE payload, the Web Animations model, spring retargets
//! and the transitions' native limits.

use presto_base::anim::{AnimValue, FillMode};
use presto_base::time::Time;
use presto_motion::op::AnimateOp;
use presto_motion::shim::spring::{SpringMotion, SvelteSpring};
use presto_motion::transition::{Channel, Computed, CssLength, NotNative, Params, Run, Transition, TransitionKind};
use presto_motion::waapi::{Animation, Animations, PlayState};

fn at_ms(ms: f64) -> Time {
    Time((ms * 1e6) as u64)
}

#[test]
fn split_css_unit_reads_like_svelte() {
    assert_eq!(CssLength::Number(12.0).split(), (12.0, "px".into()));
    assert_eq!(CssLength::Text("12".into()).split(), (12.0, "px".into()));
    assert_eq!(CssLength::Text(" -4.5em ".into()).split(), (-4.5, "em".into()));
    assert_eq!(CssLength::Text("50%".into()).split(), (50.0, "%".into()));
    assert!(CssLength::Text("auto".into()).split().0.is_nan());
}

#[test]
fn fly_resolves_percent_against_the_node_and_refuses_em() {
    let style = Computed { width: "200px".into(), height: "50px".into(), ..Computed::default() };
    let p = Params { x: Some(CssLength::Text("50%".into())), y: Some(CssLength::Text("-10%".into())), ..Params::default() };
    let t = Transition::new(TransitionKind::Fly, &p, &style).unwrap();
    let f = t.css(0.0, 1.0);
    assert_eq!(f.transform.unwrap().last().copied(), Some(presto_motion::transition::Transform64::Translate(100.0, -5.0)));
    let p = Params { x: Some(CssLength::Text("2em".into())), ..Params::default() };
    assert_eq!(Transition::new(TransitionKind::Fly, &p, &style), Err(NotNative::Unit("em".into())));
}

#[test]
fn a_custom_easing_is_not_native() {
    let p = Params { easing: Some("myEase".into()), ..Params::default() };
    assert!(matches!(Transition::new(TransitionKind::Fade, &p, &Computed::default()), Err(NotNative::UnknownEasing(_))));
}

#[test]
fn slide_opacity_track_has_its_knee() {
    let style = Computed { height: "100px".into(), ..Computed::default() };
    let t = Transition::new(TransitionKind::Slide, &Params::default(), &style).unwrap();
    let plan = t.plan(&Run::new(&t, 0.0, 1.0));
    let opacity = plan.tracks.iter().find(|t| t.channel == Channel::Opacity).unwrap();
    assert_eq!(opacity.keyframes.len(), 3);
    assert!((opacity.keyframes[1].offset - 0.05).abs() < 1e-7);
    assert_eq!(opacity.keyframes[1].value, AnimValue::Number(1.0));
    assert!(plan.overflow_hidden);
    assert_eq!(plan.timing.fill, FillMode::Forwards);
    assert!(plan.statics.iter().any(|(c, v)| *c == Channel::Css("min-height") && *v == AnimValue::Length(0.0)));
}

#[test]
fn op_prefers_the_native_descriptor_and_falls_back_to_frames() {
    let json = r#"{"id":3,"duration":400,"delay":0,
        "frames":[{"opacity":0},{"opacity":1}],
        "native":{"kind":"fade","params":{"duration":400},"computed":{"opacity":"0.5"},"t1":0,"t2":1}}"#;
    let op = AnimateOp::parse(json).unwrap();
    let plan = op.plan();
    assert_eq!(plan.tracks.len(), 1);
    assert_eq!(plan.tracks[0].keyframes[1].value, AnimValue::Number(0.5));

    // an easing presto does not know: the frames play
    let json = r#"{"id":4,"duration":100,"frames":[{"opacity":0,"transform":[0,10,1,1,0.5,0.5]},{"opacity":1,"transform":[0,0,1,1,0.5,0.5]}],
        "native":{"kind":"fade","params":{"easing":"custom"},"computed":{},"t1":0,"t2":1}}"#;
    let op = AnimateOp::parse(json).unwrap();
    assert!(matches!(op.native_plan(), Some(Err(NotNative::UnknownEasing(_)))));
    let plan = op.plan();
    assert_eq!(plan.tracks.iter().map(|t| t.channel).collect::<Vec<_>>(), vec![Channel::Opacity, Channel::Transform]);
    assert_eq!(plan.origin, Some((0.5, 0.5)));

    let cancel = AnimateOp::parse(r#"{"id":4,"cancel":true}"#).unwrap();
    assert!(cancel.cancel);
}

#[test]
fn op_plans_a_flip() {
    let json = r#"{"id":9,"duration":300,"delay":0,"frames":[],
        "native":{"kind":"flip","params":{},"computed":{"transform":"none"},"t1":0,"t2":1,
          "flip":{"from":{"left":0,"top":0,"width":100,"height":40},"to":{"left":0,"top":100,"width":100,"height":40},
                  "clientWidth":100,"clientHeight":40,"origin":[50,20]}}}"#;
    let plan = AnimateOp::parse(json).unwrap().plan();
    assert_eq!(plan.tracks.len(), 1);
    let AnimValue::Transform(start) = &plan.tracks[0].keyframes[0].value else { panic!() };
    assert_eq!(start[0], presto_base::anim::TransformFn::Translate(0.0, -100.0));
}

#[test]
fn waapi_plays_finishes_once_and_cancels() {
    let mut list: Animations<u32> = Animations::new();
    list.start(1, Animation::new(10, 0.0, 400.0, true, at_ms(1000.0)));
    list.start(1, Animation::new(11, 100.0, 0.0, true, at_ms(1000.0)));
    let a = list.get(10).unwrap();
    assert_eq!(a.play_state(at_ms(1200.0)), PlayState::Running);
    assert_eq!(a.current_time(at_ms(1200.0)), Some(200.0));
    assert_eq!(a.progress(at_ms(1200.0)), Some(0.5));
    assert_eq!(list.next_finish(), Some(at_ms(1100.0)));
    assert_eq!(list.poll(at_ms(1150.0)), vec![(1, 11)]);
    assert!(list.poll(at_ms(1160.0)).is_empty());
    assert_eq!(list.poll(at_ms(1400.0)), vec![(1, 10)]);
    // fill forwards: still in effect, at the end
    assert_eq!(list.of_node(1, at_ms(2000.0)), vec![10, 11]);
    assert_eq!(list.get(10).unwrap().current_time(at_ms(2000.0)), Some(400.0));
    assert_eq!(list.cancel(10), Some(1));
    assert_eq!(list.of_node(1, at_ms(2000.0)), vec![11]);
}

#[test]
fn waapi_cancel_effect_null_pause_and_finish() {
    let mut a = Animation::new(1, 0.0, 300.0, true, at_ms(0.0));
    a.pause(at_ms(100.0));
    assert_eq!(a.play_state(at_ms(500.0)), PlayState::Paused);
    assert_eq!(a.current_time(at_ms(500.0)), Some(100.0));
    a.play(at_ms(500.0));
    assert_eq!(a.current_time(at_ms(600.0)), Some(200.0));
    a.set_current_time(50.0, at_ms(600.0));
    assert_eq!(a.current_time(at_ms(610.0)), Some(60.0));
    a.finish();
    assert_eq!(a.play_state(at_ms(610.0)), PlayState::Finished);
    a.cancel();
    assert_eq!(a.play_state(at_ms(610.0)), PlayState::Idle);
    assert_eq!(a.current_time(at_ms(610.0)), None);
    let mut b = Animation::new(2, 0.0, 300.0, true, at_ms(0.0));
    b.clear_effect();
    assert!(!b.in_effect(at_ms(10.0)));
    assert_eq!(b.progress(at_ms(10.0)), None);
}

#[test]
fn spring_retarget_continues_like_svelte() {
    // Svelte: target 100 from 0, then at frame 7 a new target 30 (last value kept)
    let p = SvelteSpring::default();
    let (mut last, mut cur) = (0.0f64, 0.0f64);
    let mut svelte = vec![cur];
    for frame in 0..200 {
        let target = if frame < 7 { 100.0 } else { 30.0 };
        let delta = target - cur;
        let v = cur - last;
        let d = v + (p.stiffness * delta - p.damping * v);
        if d.abs() < p.precision && delta.abs() < p.precision {
            svelte.push(target);
            break;
        }
        last = cur;
        cur += d;
        svelte.push(cur);
    }
    let first = SpringMotion::new(p, 0.0, 0.0, 100.0);
    let second = first.retarget(7.0, 30.0);
    for (n, want) in svelte.iter().enumerate() {
        let got = if n <= 7 { first.value(n as f64) } else { second.value(n as f64 - 7.0) };
        assert!((got - want).abs() < 1e-6, "frame {n}: {got} vs {want}");
    }
}

#[test]
fn spring_has_a_physical_equivalent() {
    let m = SpringMotion::new(SvelteSpring { stiffness: 0.1, damping: 0.25, precision: 0.01 }, 0.0, 0.0, 100.0);
    let (k, c) = m.physical().unwrap();
    assert!(k > 0.0 && c > 0.0);
    // integrate the physical spring from the motion's start state; it follows the closed form
    let (mut x, mut v) = (m.raw(0.0) - 100.0, m.velocity(0.0));
    let dt = 1e-5;
    let steps = (0.5 / dt) as usize;
    for _ in 0..steps {
        let a = -k * x - c * v;
        v += a * dt;
        x += v * dt;
    }
    let want = m.raw(30.0) - 100.0;
    assert!((x - want).abs() < 0.05, "physical {x} vs closed form {want}");
}
