use cgmath::{Angle, EuclideanSpace, Matrix4, Point3, Rad, Vector3};
use glium::backend::glutin::SimpleWindowBuilder;
use glium::index::{IndexBuffer, PrimitiveType};
use glium::{Depth, DepthTest, DrawParameters, Program, Surface, VertexBuffer};
use rand::Rng;
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::event_loop::EventLoop;
use winit::keyboard::{Key, NamedKey};

use crate::nbody::nbody::NBodyBenchmark;
use crate::nbody::ExecutionMode;

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 3],
}

glium::implement_vertex!(Vertex, position);

fn icosahedron() -> ([Vertex; 12], Vec<u8>) {
    let phi = (1.0 + f32::sqrt(5.0)) / 2.0;

    let vertices = [
        Vertex {
            position: [phi, 1.0, 0.0],
        },
        Vertex {
            position: [phi, -1.0, 0.0],
        },
        Vertex {
            position: [-phi, 1.0, 0.0],
        },
        Vertex {
            position: [-phi, -1.0, 0.0],
        },
        Vertex {
            position: [0.0, phi, 1.0],
        },
        Vertex {
            position: [0.0, phi, -1.0],
        },
        Vertex {
            position: [0.0, -phi, 1.0],
        },
        Vertex {
            position: [0.0, -phi, -1.0],
        },
        Vertex {
            position: [1.0, 0.0, phi],
        },
        Vertex {
            position: [-1.0, 0.0, phi],
        },
        Vertex {
            position: [1.0, 0.0, -phi],
        },
        Vertex {
            position: [-1.0, 0.0, -phi],
        },
    ];

    let indices = vec![
        0, 1, 8, 0, 4, 5, 0, 5, 10, 0, 8, 4, 0, 10, 1, 1, 6, 8, 1, 7, 6, 1, 10, 7, 2, 3, 11, 2, 4,
        9, 2, 5, 4, 2, 9, 3, 2, 11, 5, 3, 6, 7, 3, 7, 11, 3, 9, 6, 4, 8, 9, 5, 11, 10, 6, 9, 8, 7,
        10, 11,
    ];

    (vertices, indices)
}

#[derive(Copy, Clone)]
struct Instance {
    color: [f32; 3],
    world_position: [f32; 3],
}

glium::implement_vertex!(Instance, color, world_position);

pub fn visualize_benchmarks(num_bodies: usize, mut mode: ExecutionMode) {
    let event_loop = EventLoop::new().unwrap();
    let (window, display) = SimpleWindowBuilder::new()
        .with_inner_size(800, 600)
        .with_title("nbody demo")
        .build(&event_loop);

    let mut benchmark = NBodyBenchmark::new(num_bodies, &mut rand::thread_rng());

    let vertex_shader_src = r#"
        #version 100

        uniform mat4 matrix;

        attribute vec3 position;
        attribute vec3 world_position;
        attribute vec3 color;

        varying lowp vec3 v_color;

        void main() {
            v_color = color;
            gl_Position = matrix * vec4(position * 0.1 + world_position * 0.0005, 1.0);
        }
    "#;

    let fragment_shader_src = r#"
        #version 100

        varying lowp vec3 v_color;

        void main() {
            gl_FragColor = vec4(v_color, 1.0);
        }
    "#;

    let program =
        Program::from_source(&display, vertex_shader_src, fragment_shader_src, None).unwrap();

    let (vertices, indices) = icosahedron();
    let vertex_buffer = VertexBuffer::new(&display, &vertices).unwrap();
    let index_buffer = IndexBuffer::new(&display, PrimitiveType::TrianglesList, &indices).unwrap();

    let mut rng = crate::seeded_rng();
    let instances: Vec<_> = (0..num_bodies)
        .map(|_| Instance {
            color: [
                rng.gen_range(0.5..1.0),
                rng.gen_range(0.5..1.0),
                rng.gen_range(0.5..1.0),
            ],
            world_position: [0.0, 0.0, 0.0],
        })
        .collect();

    let mut instance_buffer = VertexBuffer::dynamic(&display, &instances).unwrap();

    event_loop
        .run(move |event, target| match event {
            Event::AboutToWait => {
                let bodies = match mode {
                    ExecutionMode::Par => benchmark.tick_par(),
                    ExecutionMode::ParReduce => benchmark.tick_par_reduce(),
                    ExecutionMode::Seq => benchmark.tick_seq(),
                };

                let mut mapping = instance_buffer.map();
                for (body, instance) in bodies.iter().zip(mapping.iter_mut()) {
                    instance.world_position = [
                        body.position.x as f32,
                        body.position.y as f32,
                        body.position.z as f32,
                    ];
                }
                window.request_redraw();
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => target.exit(),
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            state: ElementState::Pressed,
                            logical_key,
                            ..
                        },
                    ..
                } => match logical_key {
                    Key::Named(NamedKey::Escape) => target.exit(),
                    Key::Character(s) => match s.as_str() {
                        "p" => mode = ExecutionMode::Par,
                        "r" => mode = ExecutionMode::ParReduce,
                        "s" => mode = ExecutionMode::Seq,
                        _ => (),
                    },
                    _ => (),
                },
                WindowEvent::RedrawRequested => {
                    let params = DrawParameters {
                        depth: Depth {
                            test: DepthTest::IfLess,
                            write: true,
                            ..Default::default()
                        },
                        ..Default::default()
                    };

                    let mut target = display.draw();

                    let (width, height) = target.get_dimensions();
                    let aspect = width as f32 / height as f32;

                    let proj = cgmath::perspective(Rad::full_turn() / 6.0, aspect, 0.1, 3000.0);
                    let view = Matrix4::look_at_rh(
                        Point3::new(10.0, 10.0, 10.0),
                        Point3::origin(),
                        Vector3::unit_z(),
                    );
                    let view_proj: [[f32; 4]; 4] = (proj * view).into();

                    target.clear_color_and_depth((0.1, 0.1, 0.1, 1.0), 1.0);
                    target
                        .draw(
                            (&vertex_buffer, instance_buffer.per_instance().unwrap()),
                            &index_buffer,
                            &program,
                            &glium::uniform! { matrix: view_proj },
                            &params,
                        )
                        .unwrap();
                    target.finish().unwrap();
                }
                _ => (),
            },
            _ => (),
        })
        .unwrap();
}
