//! Integration tests for Pod types.

extern crate std;
use std::vec;
use std::vec::Vec;

use bytemuck::try_from_bytes;

use crate::*;

mod option;
mod pod_bool;
mod pod_numeric;
mod pod_vec;
mod string;
