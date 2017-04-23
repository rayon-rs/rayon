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

use cgmath::{InnerSpace, Point3, Vector3, Zero};
use rayon::prelude::*;
use rand::{Rand, Rng};
use std::f64::consts::PI;

const INITIAL_VELOCITY: f64 = 8.0; // set to 0.0 to turn off.

pub struct NBodyBenchmark {
    time: usize,
    bodies: (Vec<Body>, Vec<Body>),
}

#[derive(Copy, Clone, Debug)]
pub struct Body {
    pub position: Point3<f64>,
    pub velocity: Vector3<f64>,
    pub velocity2: Vector3<f64>,
}

impl NBodyBenchmark {
    pub fn new<R: Rng>(num_bodies: usize, rng: &mut R) -> NBodyBenchmark {
        let bodies0: Vec<_> = (0..num_bodies)
            .map(
                |_| {
                    let position = Point3 {
                        x: f64::rand(rng).floor() * 40_000.0,
                        y: f64::rand(rng).floor() * 20_000.0,
                        z: (f64::rand(rng).floor() - 0.25) * 50_000.0,
                    };

                    let velocity = Vector3 {
                        x: (f64::rand(rng) - 0.5) * INITIAL_VELOCITY,
                        y: (f64::rand(rng) - 0.5) * INITIAL_VELOCITY,
                        z: f64::rand(rng) * INITIAL_VELOCITY + 10.0,
                    };

                    let velocity2 = Vector3 {
                        x: (f64::rand(rng) - 0.5) * INITIAL_VELOCITY,
                        y: (f64::rand(rng) - 0.5) * INITIAL_VELOCITY,
                        z: f64::rand(rng) * INITIAL_VELOCITY,
                    };

                    Body {
                        position: position,
                        velocity: velocity,
                        velocity2: velocity2,
                    }
                },
            )
            .collect();

        let bodies1 = bodies0.clone();

        NBodyBenchmark {
            time: 0,
            bodies: (bodies0, bodies1),
        }
    }

    pub fn tick_par(&mut self) -> &[Body] {
        let (in_bodies, out_bodies) = if (self.time & 1) == 0 {
            (&self.bodies.0, &mut self.bodies.1)
        } else {
            (&self.bodies.1, &mut self.bodies.0)
        };

        let time = self.time;
        out_bodies
            .par_iter_mut()
            .zip(&in_bodies[..])
            .for_each(
                |(out, prev)| {
                    let (vel, vel2) = next_velocity(time, prev, in_bodies);
                    out.velocity = vel;
                    out.velocity2 = vel2;

                    let next_velocity = vel - vel2;
                    out.position = prev.position + next_velocity;
                },
            );

        self.time += 1;

        out_bodies
    }

    pub fn tick_par_reduce(&mut self) -> &[Body] {
        let (in_bodies, out_bodies) = if (self.time & 1) == 0 {
            (&self.bodies.0, &mut self.bodies.1)
        } else {
            (&self.bodies.1, &mut self.bodies.0)
        };

        let time = self.time;
        out_bodies
            .par_iter_mut()
            .zip(&in_bodies[..])
            .for_each(
                |(out, prev)| {
                    let (vel, vel2) = next_velocity_par(time, prev, in_bodies);
                    out.velocity = vel;
                    out.velocity2 = vel2;

                    let next_velocity = vel - vel2;
                    out.position = prev.position + next_velocity;
                },
            );

        self.time += 1;

        out_bodies
    }

    pub fn tick_seq(&mut self) -> &[Body] {
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

        out_bodies
    }
}

/// Compute next velocity of `prev`
fn next_velocity(time: usize, prev: &Body, bodies: &[Body]) -> (Vector3<f64>, Vector3<f64>) {
    let time = time as f64;
    let center = Point3 {
        x: (time / 22.0).cos() * -4200.0,
        y: (time / 14.0).sin() * 9200.0,
        z: (time / 27.0).sin() * 6000.0,
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

    let mut acc = Vector3::zero();
    let mut acc2 = Vector3::zero();

    let dir_to_center = center - prev.position;
    let dist_to_center = dir_to_center.magnitude();

    // orient to center
    if dist_to_center > max_distance {
        let velc = if time < 200.0 {
            0.2
        } else {
            (dist_to_center - max_distance) * pull_strength
        };

        let diff = (dir_to_center / dist_to_center) * velc;
        acc += diff;
    }

    // TODO -- parallelize this loop too? would be easy enough, it's just a reduction
    // Just have to factor things so that `tick_seq` doesn't do it in parallel.
    let zero: Vector3<f64> = Vector3::zero();
    let (diff, diff2) = bodies
        .iter()
        .fold(
            (zero, zero), |(mut diff, mut diff2), body| {
                let r = body.position - prev.position;

                // make sure we are not testing the particle against its own position
                let are_same = r == Vector3::zero();

                let dist_sqrd = r.magnitude2();

                if dist_sqrd < zone_sqrd && !are_same {
                    let length = dist_sqrd.sqrt();
                    let percent = dist_sqrd / zone_sqrd;

                    if dist_sqrd < repel {
                        let f = (repel / percent - 1.0) * 0.025;
                        let normal = (r / length) * f;
                        diff += normal;
                        diff2 += normal;
                    } else if dist_sqrd < align {
                        let thresh_delta = align - repel;
                        let adjusted_percent = (percent - repel) / thresh_delta;
                        let q = (0.5 - (adjusted_percent * PI * 2.0).cos() * 0.5 + 0.5) * 100.9;

                        // normalize vel2 and multiply by factor
                        let vel2_length = body.velocity2.magnitude();
                        let vel2 = (body.velocity2 / vel2_length) * q;

                        // normalize own velocity
                        let vel_length = prev.velocity.magnitude();
                        let vel = (prev.velocity / vel_length) * q;

                        diff += vel2;
                        diff2 += vel;
                    }

                    if dist_sqrd > attract {
                        // attract
                        let thresh_delta2 = 1.0 - attract;
                        let adjusted_percent2 = (percent - attract) / thresh_delta2;
                        let c = (1.0 - ((adjusted_percent2 * PI * 2.0).cos() * 0.5 + 0.5)) *
                                attract_power;

                        // normalize the distance vector
                        let d = (r / length) * c;

                        diff += d;
                        diff2 -= d;
                    }
                }

                (diff, diff2)
            }
        );

    acc += diff;
    acc2 += diff2;

    // Speed limits
    if time > 500.0 {
        let acc_squared = acc.magnitude2();
        if acc_squared > speed_limit {
            acc *= 0.015;
        }

        let acc_squared2 = acc2.magnitude2();
        if acc_squared2 > speed_limit {
            acc2 *= 0.015;
        }
    }

    let mut new = prev.velocity + acc;
    let mut new2 = prev.velocity2 + acc2;

    if time < 500.0 {
        let acs = new2.magnitude2();
        if acs > speed_limit {
            new2 *= 0.15;
        }

        let acs2 = new.magnitude2();
        if acs2 > speed_limit {
            new *= 0.15;
        }
    }

    (new, new2)
}

/// Compute next velocity of `prev`
fn next_velocity_par(time: usize, prev: &Body, bodies: &[Body]) -> (Vector3<f64>, Vector3<f64>) {
    let time = time as f64;
    let center = Point3 {
        x: (time / 22.0).cos() * -4200.0,
        y: (time / 14.0).sin() * 9200.0,
        z: (time / 27.0).sin() * 6000.0,
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

    let mut acc = Vector3::zero();
    let mut acc2 = Vector3::zero();

    let dir_to_center = center - prev.position;
    let dist_to_center = dir_to_center.magnitude();

    // orient to center
    if dist_to_center > max_distance {
        let velc = if time < 200.0 {
            0.2
        } else {
            (dist_to_center - max_distance) * pull_strength
        };

        let diff = (dir_to_center / dist_to_center) * velc;
        acc += diff;
    }

    let (diff, diff2) = bodies
        .par_iter()
        .fold(
            || (Vector3::zero(), Vector3::zero()),
            |(mut diff, mut diff2), body| {
                let r = body.position - prev.position;

                // make sure we are not testing the particle against its own position
                let are_same = r == Vector3::zero();

                let dist_sqrd = r.magnitude2();

                if dist_sqrd < zone_sqrd && !are_same {
                    let length = dist_sqrd.sqrt();
                    let percent = dist_sqrd / zone_sqrd;

                    if dist_sqrd < repel {
                        let f = (repel / percent - 1.0) * 0.025;
                        let normal = (r / length) * f;
                        diff += normal;
                        diff2 += normal;
                    } else if dist_sqrd < align {
                        let thresh_delta = align - repel;
                        let adjusted_percent = (percent - repel) / thresh_delta;
                        let q = (0.5 - (adjusted_percent * PI * 2.0).cos() * 0.5 + 0.5) * 100.9;

                        // normalize vel2 and multiply by factor
                        let vel2_length = body.velocity2.magnitude();
                        let vel2 = (body.velocity2 / vel2_length) * q;

                        // normalize own velocity
                        let vel_length = prev.velocity.magnitude();
                        let vel = (prev.velocity / vel_length) * q;

                        diff += vel2;
                        diff2 += vel;
                    }

                    if dist_sqrd > attract {
                        // attract
                        let thresh_delta2 = 1.0 - attract;
                        let adjusted_percent2 = (percent - attract) / thresh_delta2;
                        let c = (1.0 - ((adjusted_percent2 * PI * 2.0).cos() * 0.5 + 0.5)) *
                                attract_power;

                        // normalize the distance vector
                        let d = (r / length) * c;

                        diff += d;
                        diff2 -= d;
                    }
                }

                (diff, diff2)
            },
        )
        .reduce(
            || (Vector3::zero(), Vector3::zero()),
            |(diffa, diff2a), (diffb, diff2b)| (diffa + diffb, diff2a + diff2b),
        );

    acc += diff;
    acc2 += diff2;

    // Speed limits
    if time > 500.0 {
        let acc_squared = acc.magnitude2();
        if acc_squared > speed_limit {
            acc *= 0.015;
        }

        let acc_squared2 = acc2.magnitude2();
        if acc_squared2 > speed_limit {
            acc2 *= 0.015;
        }
    }

    let mut new = prev.velocity + acc;
    let mut new2 = prev.velocity2 + acc2;

    if time < 500.0 {
        let acs = new2.magnitude2();
        if acs > speed_limit {
            new2 *= 0.15;
        }

        let acs2 = new.magnitude2();
        if acs2 > speed_limit {
            new *= 0.15;
        }
    }

    (new, new2)
}
