//! A story × the chosen axes, expanded in the stable order: variant, args (keys in order, the
//! first key slowest), state, theme, density, zoom.

use crate::model::{Cell, State, Story, Theme, VariantId};
use skelter_widgets::theme::Density;
use std::collections::BTreeMap;

/// The axes to expand a story over. `None` (variants, args, states) means the story's own.
#[derive(Clone, Debug, PartialEq)]
pub struct Axes {
    /// Only these variants (in the story's order).
    pub variants: Option<Vec<VariantId>>,
    /// Fixed args: a key given here takes this one value instead of its list.
    pub args: BTreeMap<String, serde_json::Value>,
    /// Only these states, in the story's order (states the story does not list are refused by
    /// [`Matrix::new`]).
    pub states: Option<Vec<State>>,
    pub themes: Vec<Theme>,
    pub densities: Vec<Density>,
    pub zooms: Vec<f32>,
}

impl Default for Axes {
    /// The brief's defaults: every variant, args combination and story state, all four themes, the
    /// default density, zoom 1.
    fn default() -> Axes {
        Axes { variants: None, args: BTreeMap::new(), states: None, themes: Theme::BUILTIN.to_vec(), densities: vec![Density::Default], zooms: vec![1.0] }
    }
}

/// The cells of one story over [`Axes`], in order.
#[derive(Clone, Debug)]
pub struct Matrix {
    pub story: Story,
    pub axes: Axes,
    pub cells: Vec<Cell>,
}

impl Matrix {
    /// Expands `story` over `axes`. Errors name an unknown variant, an arg the variant lacks, or a
    /// state the story doesn't list.
    pub fn new(story: &Story, axes: &Axes) -> Result<Matrix, String> {
        let variants: Vec<_> = match &axes.variants {
            Some(want) => {
                for v in want {
                    if story.variant(&v.0).is_none() {
                        return Err(format!("story {} has no variant {v} (it has {})", story.id, story.variants.iter().map(|v| v.id.0.as_str()).collect::<Vec<_>>().join(", ")));
                    }
                }
                story.variants.iter().filter(|v| want.contains(&v.id)).collect()
            }
            None => story.variants.iter().collect(),
        };
        let states: Vec<State> = match &axes.states {
            Some(want) => {
                for s in want {
                    if !story.states.contains(s) {
                        return Err(format!("story {} does not show state {s} (it lists {})", story.id, story.states.iter().map(|s| s.name()).collect::<Vec<_>>().join(", ")));
                    }
                }
                story.states.iter().copied().filter(|s| want.contains(s)).collect()
            }
            None => story.states.clone(),
        };
        if axes.themes.is_empty() || axes.densities.is_empty() || axes.zooms.is_empty() {
            return Err("every axis needs at least one value".into());
        }
        if let Some(z) = axes.zooms.iter().find(|z| !(0.25..=8.0).contains(*z)) {
            return Err(format!("zoom {z} is outside 0.25..8"));
        }
        let mut cells = Vec::new();
        for v in variants {
            let mut fixed = v.clone();
            for (k, val) in &axes.args {
                if !fixed.args.contains_key(k) && !fixed.props.contains(k) {
                    return Err(format!("variant {}/{} has no arg or prop {k}", story.id, v.id));
                }
                fixed.args.insert(k.clone(), vec![val.clone()]);
            }
            for args in fixed.arg_combinations() {
                for &state in &states {
                    for theme in &axes.themes {
                        for &density in &axes.densities {
                            for &zoom in &axes.zooms {
                                cells.push(Cell { story: story.id.clone(), variant: v.id.clone(), args: args.clone(), state, theme: theme.clone(), density, zoom });
                            }
                        }
                    }
                }
            }
        }
        Ok(Matrix { story: story.clone(), axes: axes.clone(), cells })
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}
