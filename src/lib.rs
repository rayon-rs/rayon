#![feature(unsafe_destructor)]

use std::any::Any;
use std::kinds::marker;
use std::mem::transmute;
use std::raw;
use std::sync::Future;
use std::task;

