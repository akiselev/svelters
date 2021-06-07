use cpp::cpp;
use qmetaobject::*;

fn object_name_changed() -> Signal<fn(QString)> {
    unsafe {
        Signal::new(cpp!([] -> SignalInner as "SignalInner"  {
            return &QObject::objectNameChanged;
        }))
    }
}
