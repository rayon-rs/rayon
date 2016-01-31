use nbody::NBodyBenchmark;
use rand::{self, Rng, SeedableRng};
use sdl;
use sdl::video::{SurfaceFlag, VideoFlag};
use sdl::event::{Event, Key};

pub fn visualize_benchmarks(num_bodies: usize) {
    let mut benchmark = NBodyBenchmark::new(num_bodies, &mut rand::thread_rng());

    sdl::init(&[sdl::InitFlag::Video]);
    sdl::wm::set_caption("nbody demo", "nbody");
    let screen = match sdl::video::set_video_mode(800, 600, 32,
                                                  &[SurfaceFlag::HWSurface],
                                                  &[VideoFlag::DoubleBuf]) {
        Ok(screen) => screen,
        Err(err) => panic!("failed to set video mode: {}", err)
    };

    let mut par_mode = true;

    loop {
        match sdl::event::poll_event() {
            Event::Quit => break,
            Event::None => { }
            Event::Key(k, _, _, _) if k == Key::Escape => break,
            _ => {}
        }

        let bodies = if par_mode {
            benchmark.tick_par()
        } else {
            benchmark.tick_seq()
        };

        let mut color_rng = rand::XorShiftRng::from_seed([0, 1, 2, 3]);
        screen.clear();
        for body in bodies {
            // Project 3d point onto 2d screen. I really ought to be using OpenGL.
            let b_z = -3000.0;
            let a_z = body.position.z;
            let x = (body.position.x) * b_z / a_z;
            let y = (body.position.y) * b_z / a_z;
            let size = 10.0 * b_z / a_z;
            screen.fill_rect(Some(sdl::Rect {
                x: (x as i16) + 400,
                y: (y as i16) + 300,
                w: size as u16,
                h: size as u16,
            }), color_rng.gen::<sdl::video::Color>());
        }

        screen.flip();
    }

    sdl::quit();
}

