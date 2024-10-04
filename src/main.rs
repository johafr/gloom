#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(unreachable_code)]
#![allow(unused_mut)]
#![allow(unused_unsafe)]
#![allow(unused_variables)]
#![allow(unused_assignments)]

extern crate nalgebra_glm as glm;
use std::mem::ManuallyDrop;
use std::pin::Pin;
use std::{ mem, ptr, os::raw::c_void };
use std::thread;
use std::sync::{Mutex, Arc, RwLock};
use std::time::Instant;

mod shader;
mod util;
mod mesh;
mod scene_graph;
mod toolbox;

use scene_graph::SceneNode;

use gl::types::GLuint;
use glutin::event::{Event, WindowEvent, DeviceEvent, KeyboardInput, ElementState::{Pressed, Released}, VirtualKeyCode::{self, *}};
use glutin::event_loop::ControlFlow;
use mesh::Helicopter;


// initial window size
const INITIAL_SCREEN_W: u32 = 800;
const INITIAL_SCREEN_H: u32 = 600;

// == // Helper functions to make interacting with OpenGL a little bit prettier. You *WILL* need these! // == //

// Get the size of an arbitrary array of numbers measured in bytes
// Example usage:  byte_size_of_array(my_array)
fn byte_size_of_array<T>(val: &[T]) -> isize {
    std::mem::size_of_val(&val[..]) as isize
}

// Get the OpenGL-compatible pointer to an arbitrary array of numbers
// Example usage:  pointer_to_array(my_array)
fn pointer_to_array<T>(val: &[T]) -> *const c_void {
    &val[0] as *const T as *const c_void
}

// Get the size of the given type in bytes
// Example usage:  size_of::<u64>()
fn size_of<T>() -> i32 {
    mem::size_of::<T>() as i32
}

// Get an offset in bytes for n units of type T, represented as a relative pointer
// Example usage:  offset::<u64>(4)
fn offset<T>(n: u32) -> *const c_void {
    (n * mem::size_of::<T>() as u32) as *const T as *const c_void
}


unsafe fn create_vao(vertices: &Vec<f32>, indices: &Vec<u32>, colours: &Vec<f32>, normals: &Vec<f32>) -> u32 {
    // constants
    let mut vao: GLuint = 0;
    let mut vbo: GLuint = 0;
    let mut ibo: GLuint = 0;

    // Concat vertices and colours
    let mut vectors: Vec<f32> = Vec::new();

    // Converting vertices and colours into their own vectors
    let chunked_vertices: Vec<Vec<f32>> = vertices.chunks(3).map(|chunk| chunk.to_vec()).collect();
    let chunked_colours: Vec<Vec<f32>> = colours.chunks(4).map(|chunk| chunk.to_vec()).collect();
    let chunked_normals: Vec<Vec<f32>> = normals.chunks(3).map(|chunk| chunk.to_vec()).collect();

    // Iterating over all vertices and colours and adding each vertex-colour object and normal-vector object to vectors on the form [X, Y, Z, R, G, B, A, X, Y, Z]
    for i in 0..chunked_vertices.len() {
        vectors.extend(&chunked_vertices[i]);
        vectors.extend(&chunked_colours[i]);
        vectors.extend(&chunked_normals[i]);
    }

    // * Generate a VAO and bind it
    gl::GenVertexArrays(1, &mut vao);
    gl::BindVertexArray(vao);

    // * Generate a VBO and bind it
    gl::GenBuffers(1, &mut vbo);
    gl::BindBuffer(gl::ARRAY_BUFFER,vbo);

    // * Fill it with data
    gl::BufferData(
        gl::ARRAY_BUFFER,
        byte_size_of_array(&vectors), 
        pointer_to_array(&vectors),
        gl::STATIC_DRAW
    );

    // * Configure a VAP for the data and enable it
    // Pos
    gl::VertexAttribPointer(
        0,
        3,
        gl::FLOAT,
        gl::FALSE,
        10 * size_of::<f32>(),
        offset::<f32>(0)
    );
    gl::EnableVertexAttribArray(0);

    // Colour
    gl::VertexAttribPointer(
        1,
        4,
        gl::FLOAT,
        gl::FALSE,
        10 * size_of::<f32>(),
        offset::<f32>(3)
    );
    gl::EnableVertexAttribArray(1);

    // Normals
    gl::VertexAttribPointer(
        2,
        3,
        gl::FLOAT,
        gl::FALSE,
        10 * size_of::<f32>(),
        offset::<f32>(7)
    );
    gl::EnableVertexAttribArray(2);


    // * Generate a IBO and bind it
    gl::GenBuffers(1, &mut ibo);
    gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ibo);

    // * Fill it with data
    gl::BufferData(
        gl::ELEMENT_ARRAY_BUFFER,
        byte_size_of_array(&indices),
        pointer_to_array(&indices),
        gl::STATIC_DRAW
    );

    // Unbind VAO
    gl::BindVertexArray(0);

    // * Return the ID of the VAO
    vao
}

fn main() {
    // Set up the necessary objects to deal with windows and event handling
    let el = glutin::event_loop::EventLoop::new();
    let wb = glutin::window::WindowBuilder::new()
        .with_title("Gloom-rs")
        .with_resizable(true)
        .with_inner_size(glutin::dpi::LogicalSize::new(INITIAL_SCREEN_W, INITIAL_SCREEN_H));
    let cb = glutin::ContextBuilder::new()
        .with_vsync(true);
    let windowed_context = cb.build_windowed(wb, &el).unwrap();
    // Uncomment these if you want to use the mouse for controls, but want it to be confined to the screen and/or invisible.
    // windowed_context.window().set_cursor_grab(true).expect("failed to grab cursor");
    // windowed_context.window().set_cursor_visible(false);

    // Set up a shared vector for keeping track of currently pressed keys
    let arc_pressed_keys = Arc::new(Mutex::new(Vec::<VirtualKeyCode>::with_capacity(10)));
    // Make a reference of this vector to send to the render thread
    let pressed_keys = Arc::clone(&arc_pressed_keys);

    // Set up shared tuple for tracking mouse movement between frames
    let arc_mouse_delta = Arc::new(Mutex::new((0f32, 0f32)));
    // Make a reference of this tuple to send to the render thread
    let mouse_delta = Arc::clone(&arc_mouse_delta);

    // Set up shared tuple for tracking changes to the window size
    let arc_window_size = Arc::new(Mutex::new((INITIAL_SCREEN_W, INITIAL_SCREEN_H, false)));
    // Make a reference of this tuple to send to the render thread
    let window_size = Arc::clone(&arc_window_size);

    // Spawn a separate thread for rendering, so event handling doesn't block rendering
    let render_thread = thread::spawn(move || {
        // Acquire the OpenGL Context and load the function pointers.
        // This has to be done inside of the rendering thread, because
        // an active OpenGL context cannot safely traverse a thread boundary
        let context = unsafe {
            let c = windowed_context.make_current().unwrap();
            gl::load_with(|symbol| c.get_proc_address(symbol) as *const _);
            c
        };

        let mut window_aspect_ratio = INITIAL_SCREEN_W as f32 / INITIAL_SCREEN_H as f32;
       
        // Camera variables a)
        let mut camera_position = glm::vec3(0.0, 0.0, 0.0);
        let mut pitch: f32 = 0.0;
        let mut yaw: f32 = 0.0;
        let camera_speed: f32 = 30.0;

        // Set up openGL
        unsafe {
            gl::Enable(gl::DEPTH_TEST);
            gl::DepthFunc(gl::LESS);
            gl::Enable(gl::CULL_FACE);
            gl::Disable(gl::MULTISAMPLE);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::Enable(gl::DEBUG_OUTPUT_SYNCHRONOUS);
            gl::DebugMessageCallback(Some(util::debug_callback), ptr::null());

            // Print some diagnostics
            //println!("{}: {}", util::get_gl_string(gl::VENDOR), util::get_gl_string(gl::RENDERER));
            //println!("OpenGL\t: {}", util::get_gl_string(gl::VERSION));
            //println!("GLSL\t: {}", util::get_gl_string(gl::SHADING_LANGUAGE_VERSION));
        }

        // == // Set up your VAO around here
        let terrain_path: &str = "./resources/lunarsurface.obj";
        let lunarsurface: mesh::Mesh = mesh::Terrain::load(&terrain_path);

        let vehicle_path: &str = "./resources/helicopter.obj";
        let helicopter: Helicopter = mesh::Helicopter::load(&vehicle_path);

        let mut terrain_vao = unsafe {create_vao(&lunarsurface.vertices, &lunarsurface.indices, &lunarsurface.colors, &lunarsurface.normals)};
        let mut body_vao = unsafe {create_vao(&helicopter.body.vertices, &helicopter.body.indices, &helicopter.body.colors, &helicopter.body.normals)};
        let mut door_vao = unsafe {create_vao(&helicopter.door.vertices, &helicopter.door.indices, &helicopter.door.colors, &helicopter.door.normals)};
        let mut main_rotor_vao = unsafe {create_vao(&helicopter.main_rotor.vertices, &helicopter.main_rotor.indices, &helicopter.main_rotor.colors, &helicopter.main_rotor.normals)};
        let mut tail_rotor_vao = unsafe {create_vao(&helicopter.tail_rotor.vertices, &helicopter.tail_rotor.indices, &helicopter.tail_rotor.colors, &helicopter.tail_rotor.normals)};

        let mut parent_node = SceneNode::new();
        let mut terrain_node = SceneNode::from_vao(terrain_vao, lunarsurface.index_count);
        let mut helicopter_body_node = SceneNode::from_vao(body_vao, helicopter.body.index_count);
        let mut helicopter_door_node = SceneNode::from_vao(door_vao, helicopter.door.index_count);
        let mut helicopter_main_rotor_node = SceneNode::from_vao(main_rotor_vao, helicopter.main_rotor.index_count);
        let mut helicopter_tail_rotor_node = SceneNode::from_vao(tail_rotor_vao, helicopter.tail_rotor.index_count);
        
        terrain_node.reference_point = glm::vec3(0.0, 0.0, 0.0);
        helicopter_body_node.reference_point = glm::vec3(0.0, 0.0, 0.0);
        helicopter_door_node.reference_point = glm::vec3(0.0, 0.0, 0.0);
        helicopter_main_rotor_node.reference_point = glm::vec3(0.0, 0.0, 0.0);
        helicopter_tail_rotor_node.reference_point = glm::vec3(-0.35, -2.3, -10.4);
        
        parent_node.add_child(&terrain_node);
        terrain_node.add_child(&helicopter_body_node);
        helicopter_body_node.add_child(&helicopter_door_node);
        helicopter_body_node.add_child(&helicopter_main_rotor_node);
        helicopter_body_node.add_child(&helicopter_tail_rotor_node);
        
        //helicopter_body_node.position = glm::vec3(0.0, 0.0, -20.0);
        // == // Set up your shaders here

        // Basic usage of shader helper:
        // The example code below creates a 'shader' object.
        // It which contains the field `.program_id` and the method `.activate()`.
        // The `.` in the path is relative to `Cargo.toml`.
        // This snippet is not enough to do the exercise, and will need to be modified (outside
        // of just using the correct path), but it only needs to be called once

        
        let simple_shader = unsafe {
            shader::ShaderBuilder::new()
                .attach_file("./shaders/simple.vert")
                .attach_file("./shaders/simple.frag")
                .link()
        };

        // The main rendering loop
        let first_frame_time = std::time::Instant::now();
        let mut previous_frame_time = first_frame_time;
        loop {
            // Compute time passed since the previous frame and since the start of the program
            let now = Instant::now();
            let elapsed = now.duration_since(first_frame_time).as_secs_f32();
            let delta_time = now.duration_since(previous_frame_time).as_secs_f32();
            previous_frame_time = now;

            // Handle resize events
            if let Ok(mut new_size) = window_size.lock() {
                if new_size.2 {
                    context.resize(glutin::dpi::PhysicalSize::new(new_size.0, new_size.1));
                    window_aspect_ratio = new_size.0 as f32 / new_size.1 as f32;
                    (*new_size).2 = false;
                    println!("Window was resized to {}x{}", new_size.0, new_size.1);
                    unsafe { gl::Viewport(0, 0, new_size.0 as i32, new_size.1 as i32); }
                }
            }

            // Handle keyboard input
            if let Ok(keys) = pressed_keys.lock() {
                for key in keys.iter() {
                    match key {
                        // The `VirtualKeyCode` enum is defined here:
                        //    https://docs.rs/winit/0.25.0/winit/event/enum.VirtualKeyCode.html
                        
                        VirtualKeyCode::W => {
                            camera_position.z += delta_time * camera_speed;
                        }
                        VirtualKeyCode::S => {
                            camera_position.z -= delta_time * camera_speed;
                        }
                        VirtualKeyCode::D => {
                            camera_position.x -= delta_time * camera_speed;
                        }
                        VirtualKeyCode::A => {
                            camera_position.x += delta_time * camera_speed;
                        }
                        VirtualKeyCode::Space => {
                            camera_position.y += delta_time * camera_speed;
                        }
                        VirtualKeyCode::LShift => {
                            camera_position.y -= delta_time * camera_speed;
                        }
                        VirtualKeyCode::Up => {
                            pitch -= delta_time * 3.0;
                        }
                        VirtualKeyCode::Down => {
                            pitch += delta_time * 3.0;
                        }
                        VirtualKeyCode::Right => {
                            yaw += delta_time * 3.0;
                        }
                        VirtualKeyCode::Left => {
                            yaw -= delta_time * 3.0;
                        }
                        // default handler:
                        _ => { }
                    }
                    
                }
            }

            // Handle mouse movement. delta contains the x and y movement of the mouse since last frame in pixels
            if let Ok(mut delta) = mouse_delta.lock() {

                // == // Optionally access the accumulated mouse movement between
                // == // frames here with `delta.0` and `delta.1`

                *delta = (0.0, 0.0); // reset when done
            }

            // == // Please compute camera transforms here (exercise 2 & 3)
            let mut view_matrix: glm::Mat4 = glm::identity();

            let fovy = 45.0_f32.to_radians();

            let perspective_transform = glm::perspective(window_aspect_ratio, fovy, 0.1, 1000.0);
            let position_transform = glm::translation(&camera_position);

            let yaw_rotation = glm::rotation(yaw, &glm::vec3(0.0, 1.0, 0.0));
            let pitch_rotation = glm::rotation(pitch, &glm::vec3(1.0, 0.0, 0.0));

            view_matrix = perspective_transform * pitch_rotation * yaw_rotation * view_matrix * position_transform;

            //Updating the rotors:
            helicopter_main_rotor_node.rotation = glm::vec3(0.0, elapsed * 5.0, 0.0);
            helicopter_tail_rotor_node.rotation = glm::vec3(elapsed * 5.0, 0.0 , 0.0);

            let delta_pose = toolbox::simple_heading_animation(elapsed);

            helicopter_body_node.position = glm::vec3(delta_pose.x, 0.0, delta_pose.z);
            helicopter_body_node.rotation = glm::vec3(delta_pose.pitch, delta_pose.yaw, delta_pose.roll);

            unsafe {
                simple_shader.activate();
                
                // Clear the color and depth buffers
                gl::ClearColor(0.035, 0.046, 0.078, 1.0); // night sky
                gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

                unsafe fn draw_scene(node: &SceneNode, view_projection_matrix: &glm::Mat4, transformation_this_far: &glm::Mat4, shader: &shader::Shader) {
                    let mut node_transformation = glm::identity::<f32, 4>();

                    let to_ref = glm::translation(&node.reference_point);
                    let from_ref = glm::translation(&-node.reference_point);

                    let roll = glm::rotation(node.rotation.x, &glm::vec3(1.0, 0.0, 0.0));
                    let pitch = glm::rotation(node.rotation.y, &glm::vec3(0.0, 1.0, 0.0));
                    let yaw = glm::rotation(node.rotation.z, &glm::vec3(0.0, 0.0, 1.0));

                    let position_transform = glm::translation(&node.position);
                    let scale_transform = glm::scaling(&node.scale);

                    node_transformation = position_transform * from_ref * scale_transform * pitch * roll * yaw * to_ref;
                    
                    if node.vao_id != 0 {
                        shader.activate();
                        gl::UniformMatrix4fv(shader.get_uniform_location("mvp_matrix"), 1, gl::FALSE, glm::value_ptr(&(view_projection_matrix*transformation_this_far*node_transformation)).as_ptr());
                        gl::UniformMatrix4fv(shader.get_uniform_location("model_matrix"), 1, gl::FALSE, glm::value_ptr(&(transformation_this_far * node_transformation)).as_ptr());
                        gl::BindVertexArray(node.vao_id);
                        gl::DrawElements(gl::TRIANGLES, node.index_count, gl::UNSIGNED_INT, offset::<f32>(0));
                    }
                    
                    for &child in &node.children {
                        draw_scene(&*child, view_projection_matrix, &(transformation_this_far*node_transformation), shader);
                    }
                }   

                draw_scene(&parent_node, &view_matrix, &glm::identity(), &simple_shader);
            }

            // Display the new color buffer on the display
            context.swap_buffers().unwrap(); // we use "double buffering" to avoid artifacts
        }
    });


    // == //
    // == // From here on down there are only internals.
    // == //


    // Keep track of the health of the rendering thread
    let render_thread_healthy = Arc::new(RwLock::new(true));
    let render_thread_watchdog = Arc::clone(&render_thread_healthy);
    thread::spawn(move || {
        if !render_thread.join().is_ok() {
            if let Ok(mut health) = render_thread_watchdog.write() {
                println!("Render thread panicked!");
                *health = false;
            }
        }
    });

    // Start the event loop -- This is where window events are initially handled
    el.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        // Terminate program if render thread panics
        if let Ok(health) = render_thread_healthy.read() {
            if *health == false {
                *control_flow = ControlFlow::Exit;
            }
        }

        match event {
            Event::WindowEvent { event: WindowEvent::Resized(physical_size), .. } => {
                //println!("New window size received: {}x{}", physical_size.width, physical_size.height);
                if let Ok(mut new_size) = arc_window_size.lock() {
                    *new_size = (physical_size.width, physical_size.height, true);
                }
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *control_flow = ControlFlow::Exit;
            }
            // Keep track of currently pressed keys to send to the rendering thread
            Event::WindowEvent { event: WindowEvent::KeyboardInput {
                    input: KeyboardInput { state: key_state, virtual_keycode: Some(keycode), .. }, .. }, .. } => {

                if let Ok(mut keys) = arc_pressed_keys.lock() {
                    match key_state {
                        Released => {
                            if keys.contains(&keycode) {
                                let i = keys.iter().position(|&k| k == keycode).unwrap();
                                keys.remove(i);
                            }
                        },
                        Pressed => {
                            if !keys.contains(&keycode) {
                                keys.push(keycode);
                            }
                        }
                    }
                }

                // Handle Escape and Q keys separately
                match keycode {
                    Escape => { *control_flow = ControlFlow::Exit; }
                    Q      => { *control_flow = ControlFlow::Exit; }
                    _      => { }
                }
            }
            Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta }, .. } => {
                // Accumulate mouse movement
                if let Ok(mut position) = arc_mouse_delta.lock() {
                    *position = (position.0 + delta.0 as f32, position.1 + delta.1 as f32);
                }
            }
            _ => { }
        }
    });
}
