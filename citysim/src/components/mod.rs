//! Plain-data components and the enums they share. Every component is stored
//! as `Vec<Option<T>>` on [`crate::World`], indexed by `EntityId.index`.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use crate::entity::EntityId;
use crate::exec::ExecState;
use crate::goap::actions::{ActionKind, Plan};
use crate::time::Tick;
use crate::utility::{Consideration, ThinkTrace};

mod asset;
mod body;
mod building;
mod corp;
mod district;
mod gang;
mod geo;
mod kinds;
mod life;
mod mind;
mod person;
mod politics;
mod roles;
mod social;
mod status;
mod trace;

pub use asset::*;
pub use body::*;
pub use building::*;
pub use corp::*;
pub use district::*;
pub use gang::*;
pub use geo::*;
pub use kinds::*;
pub use life::*;
pub use mind::*;
pub use person::*;
pub use politics::*;
pub use roles::*;
pub use social::*;
pub use status::*;
pub use trace::*;
