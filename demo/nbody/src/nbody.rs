// Rust source (c) 2016 by the Rayon developers. This is ported from
// [JavaScript sources][1] developed by Intel as part of the Parallel
// JS project. The copyright for the original source is reproduced
// below.
//
// Copyright (c) 2011, Intel Corporation
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// - Redistributions of source code must retain the above copyright notice,
//   this list of conditions and the following disclaimer.
// - Redistributions in binary form must reproduce the above copyright notice,
//   this list of conditions and the following disclaimer in the documentation
//   and/or other materials provided with the distribution.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF
// THE POSSIBILITY OF SUCH DAMAGE.
//
// [1]: https://github.com/IntelLabs/RiverTrail/blob/master/examples/nbody-webgl/NBody.js

use rayon::par_iter::*;
use rand::{Rand, Rng};
use std::ops;

const INITIAL_VELOCITY: f64 = 8.0; // set to 0.0 to turn off.

pub struct NBodyBenchmark {
    time: usize,
    bodies: (Vec<Body>, Vec<Body>),
}

#[derive(Copy, Clone, Debug)]
struct Body {
    position: Vector,
    velocity: Vector,
    velocity2: Vector,
}

#[derive(Copy, Clone, Debug)]
struct Vector {
    x: f64,
    y: f64,
    z: f64,
}

impl NBodyBenchmark {
    pub fn new<R: Rng>(num_bodies: usize, rng: &mut R) -> NBodyBenchmark {
        let bodies0: Vec<_> =
            (0..num_bodies)
            .map(|_| {
                let position = Vector {
                    x: f64::rand(rng).floor() * 40_000.0,
                    y: f64::rand(rng).floor() * 20_000.0,
                    z: (f64::rand(rng).floor() - 0.25) * 50_000.0,
                };

                let velocity = Vector {
                    x: (f64::rand(rng) - 0.5) * INITIAL_VELOCITY,
                    y: (f64::rand(rng) - 0.5) * INITIAL_VELOCITY,
                    z: f64::rand(rng) * INITIAL_VELOCITY + 10.0,
                };

                let velocity2 = Vector {
                    x: (f64::rand(rng) - 0.5) * INITIAL_VELOCITY,
                    y: (f64::rand(rng) - 0.5) * INITIAL_VELOCITY,
                    z: f64::rand(rng) * INITIAL_VELOCITY
                };

                Body { position: position, velocity: velocity, velocity2: velocity2 }
            })
            .collect();

        let bodies1 = bodies0.clone();

        NBodyBenchmark {
            time: 0,
            bodies: (bodies0, bodies1),
        }
    }

    pub fn tick_par(&mut self) {
        let (in_bodies, out_bodies) = if (self.time & 1) == 0 {
            (&self.bodies.0, &mut self.bodies.1)
        } else {
            (&self.bodies.1, &mut self.bodies.0)
        };

        let time = self.time;
        out_bodies.par_iter_mut()
                  .weight(200.0)
                  .zip(&in_bodies[..])
                  .for_each(|(out, prev)| {
                      let (vel, vel2) = next_velocity(time, prev, in_bodies);
                      out.velocity = vel;
                      out.velocity2 = vel2;

                      let next_velocity = vel - vel2;
                      out.position = prev.position + next_velocity;
                  });

        self.time += 1;
    }

    pub fn tick_seq(&mut self) {
        let (in_bodies, out_bodies) = if (self.time & 1) == 0 {
            (&self.bodies.0, &mut self.bodies.1)
        } else {
            (&self.bodies.1, &mut self.bodies.0)
        };

        let time = self.time;
        for (out, prev) in out_bodies.iter_mut().zip(&in_bodies[..]) {
            let (vel, vel2) = next_velocity(time, prev, in_bodies);
            out.velocity = vel;
            out.velocity2 = vel2;

            let next_velocity = vel - vel2;
            out.position = prev.position + next_velocity;
        }

        self.time += 1;
    }
}

/// Compute next velocity of `prev`
fn next_velocity(time: usize, prev: &Body, bodies: &[Body]) -> (Vector, Vector) {
    let time = time as f64;
    let center = Vector {
        x: (time / 22.0).cos() * -4200.0,
        y: (time / 14.0).sin() * 9200.0,
        z: (time / 27.0).sin() * 6000.0
    };

    // pull to center
    let max_distance = 3400.0;
    let pull_strength = 0.042;

    // zones
    let zone = 400.0;
    let repel = 100.0;
    let align = 300.0;
    let attract = 100.0;

    let (speed_limit, attract_power);
    if time < 500.0 {
        speed_limit = 2000.0;
        attract_power = 100.9;
    } else {
        speed_limit = 0.2;
        attract_power = 20.9;
    }

    let zone_sqrd = 3.0 * (zone * zone);

    let mut acc = ZERO;
    let mut acc2 = ZERO;

    let dir_to_center = center - prev.position;
    let dist_to_center = dir_to_center.length();

    // orient to center
    if dist_to_center > max_distance {
        let velc = if time < 200.0 {
            0.2
        } else {
            (dist_to_center - max_distance) * pull_strength
        };

        let diff = (dir_to_center / dist_to_center) * velc;
        acc.increment(diff);
    }

    // TODO -- parallelize this loop too? would be easy enough, it's just a reduction
    // Just have to factor things so that `tick_seq` doesn't do it in parallel.
    for body in bodies {
        let r = body.position - prev.position;

        // make sure we are not testing the particle against its own position
        let are_same = r.is_zero();

        let dist_sqrd = r.length_squared();

        if dist_sqrd < zone_sqrd && !are_same {
            let length = dist_sqrd.sqrt();
            let percent = dist_sqrd / zone_sqrd;

            if dist_sqrd < repel {
                let f = (repel / percent - 1.0) * 0.025;
                let normal = (r / length) * f;
                acc.increment(normal);
                acc2.increment(normal);
            } else if dist_sqrd < align {
                let thresh_delta = align - repel;
                let adjusted_percent = (percent - repel) / thresh_delta;
                let q = (0.5 - (adjusted_percent * 3.14159265 * 2.0).cos() * 0.5 + 0.5) * 100.9;

                // normalize vel2 and multiply by factor
                let vel2_length = body.velocity2.length();
                let vel2 = (body.velocity2 / vel2_length) * q;

                // normalize own velocity
                let vel_length = prev.velocity.length();
                let vel = (prev.velocity / vel_length) * q;

                acc.increment(vel2);
                acc2.increment(vel);
            }

            if dist_sqrd > attract { // attract
                let thresh_delta2 = 1.0 - attract;
                let adjusted_percent2 = (percent - attract) / thresh_delta2;
                let c = (1.0 - ((adjusted_percent2 * 3.14159265 * 2.0).cos() * 0.5 + 0.5)) * attract_power;

                // normalize the distance vector
                let d = (r / length) * c;

                acc.increment(d);
                acc2.decrement(d);
            }
        }
    }

    // Speed limits
    if time > 500.0 {
        let acc_squared = acc.length_squared();
        if acc_squared > speed_limit {
            acc.scale(0.015);
        }

        let acc_squared2 = acc2.length_squared();
        if acc_squared2 > speed_limit {
            acc2.scale(0.015);
        }
    }

    let mut new = prev.velocity + acc;
    let mut new2 = prev.velocity2 + acc2;

    if time < 500.0 {
        let acs = new2.length_squared();
        if acs > speed_limit {
            new2.scale(0.15);
        }

        let acs2 = new.length_squared();
        if acs2 > speed_limit {
            new.scale(0.15);
        }
    }

    (new, new2)
}

const ZERO: Vector = Vector { x: 0.0, y: 0.0, z: 0.0 };

impl Vector {
    fn length_squared(&self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    fn length(&self) -> f64 {
        self.length_squared().sqrt()
    }

    fn increment(&mut self, v: Vector) {
        self.x += v.x;
        self.y += v.y;
        self.z += v.z;
    }

    fn decrement(&mut self, v: Vector) {
        self.x -= v.x;
        self.y -= v.y;
        self.z -= v.z;
    }

    fn scale(&mut self, scale: f64) {
        self.x *= scale;
        self.y *= scale;
        self.z *= scale;
    }

    fn is_zero(&self) -> bool {
        self.x == 0.0 && self.y == 0.0 && self.z == 0.0
    }
}

impl ops::Div<f64> for Vector {
    type Output = Vector;

    fn div(self, rhs: f64) -> Vector {
        Vector { x: self.x / rhs,
                 y: self.y / rhs,
                 z: self.z / rhs }
    }
}

impl ops::Mul<f64> for Vector {
    type Output = Vector;

    fn mul(self, rhs: f64) -> Vector {
        Vector { x: self.x * rhs,
                 y: self.y * rhs,
                 z: self.z * rhs }
    }
}

impl ops::Sub<Vector> for Vector {
    type Output = Vector;

    fn sub(self, rhs: Vector) -> Vector {
        Vector { x: self.x - rhs.x,
                 y: self.y - rhs.y,
                 z: self.z - rhs.z }
    }
}

impl ops::Add<Vector> for Vector {
    type Output = Vector;

    fn add(self, rhs: Vector) -> Vector {
        Vector { x: self.x + rhs.x,
                 y: self.y + rhs.y,
                 z: self.z + rhs.z }
    }
}
