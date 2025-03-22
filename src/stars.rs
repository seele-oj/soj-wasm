use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    window, HtmlCanvasElement, WebGlBuffer, WebGlProgram, WebGlRenderingContext as GL,
    WebGlShader,
};
use std::rc::Rc;
use std::cell::RefCell;


#[wasm_bindgen]
pub struct StarField {
    gl: GL,
    canvas: HtmlCanvasElement,
    stars: Vec<Star>,
    star_buffer: WebGlBuffer,
    resolution: (f32, f32),
    background_program: WebGlProgram,
    star_program: WebGlProgram,
    background_buffer: WebGlBuffer,
    meteors: Vec<Meteor>,
    meteor_buffer: WebGlBuffer,
    meteor_program: WebGlProgram,
}

struct Star {
    x: f32,
    y: f32,
    radius: f32,
    vx: f32,
    vy: f32,
    base_alpha: f32,
    twinkle_phase: f32,
    twinkle_speed: f32,
    alpha: f32,       
    color: [f32; 3],
}

struct Meteor {
    x: f32, 
    y: f32,
    vx: f32,
    vy: f32,
    lifetime: f32,
    max_lifetime: f32,
    point_size: f32,
    color: [f32; 3],
}

const METEOR_TRAIL_LENGTH: f32 = 300.0;
const METEOR_WIDTH: f32 = 0.5;

impl StarField {
    pub fn new(canvas_id: &str, num_stars: usize) -> StarField {
        let document = window().unwrap().document().unwrap();
        let canvas = document
            .get_element_by_id(canvas_id)
            .expect("Canvas element not found")
            .dyn_into::<HtmlCanvasElement>()
            .unwrap();

        let dpr = window().unwrap().device_pixel_ratio() as f32;
        let css_width = canvas.client_width() as f32;
        let css_height = canvas.client_height() as f32;
        let width = css_width * dpr;
        let height = css_height * dpr;
        canvas.set_width(width as u32);
        canvas.set_height(height as u32);
        let resolution = (width, height);

        let gl: GL = canvas
            .get_context("webgl")
            .unwrap()
            .unwrap()
            .dyn_into()
            .unwrap();

        let star_buffer = gl.create_buffer().expect("Failed to create star buffer");
        let background_buffer = gl.create_buffer().expect("Failed to create background buffer");
        let meteor_buffer = gl.create_buffer().expect("Failed to create meteor buffer");

        let mut stars = Vec::with_capacity(num_stars);
        Self::init_stars(&mut stars, num_stars, width, height);

        let meteors = Vec::new();

        let background_vertex_shader_source = r#"
            attribute vec2 a_position;
            attribute vec3 a_color;
            varying vec3 v_color;
            void main() {
                gl_Position = vec4(a_position, 0.0, 1.0);
                v_color = a_color;
            }
        "#;
        let background_fragment_shader_source = r#"
            precision mediump float;
            varying vec3 v_color;
            void main() {
                gl_FragColor = vec4(v_color, 1.0);
            }
        "#;
        let background_vertex_shader = compile_shader(&gl, GL::VERTEX_SHADER, background_vertex_shader_source)
            .expect("Background vertex shader compile error");
        let background_fragment_shader = compile_shader(&gl, GL::FRAGMENT_SHADER, background_fragment_shader_source)
            .expect("Background fragment shader compile error");
        let background_program = link_program(&gl, &background_vertex_shader, &background_fragment_shader)
            .expect("Background program link error");

        let star_vertex_shader_source = r#"
            attribute vec2 a_position;
            attribute float a_pointSize;
            attribute float a_alpha;
            attribute vec3 a_color;
            uniform vec2 u_resolution;
            varying float v_alpha;
            varying vec3 v_color;
            void main() {
                vec2 zeroToOne = a_position / u_resolution;
                vec2 zeroToTwo = zeroToOne * 2.0;
                vec2 clipSpace = zeroToTwo - 1.0;
                clipSpace.y = -clipSpace.y;
                gl_Position = vec4(clipSpace, 0.0, 1.0);
                gl_PointSize = a_pointSize;
                v_alpha = a_alpha;
                v_color = a_color;
            }
        "#;
        let star_fragment_shader_source = r#"
            precision mediump float;
            varying float v_alpha;
            varying vec3 v_color;
            void main() {
                gl_FragColor = vec4(v_color, v_alpha);
            }
        "#;
        let star_vertex_shader = compile_shader(&gl, GL::VERTEX_SHADER, star_vertex_shader_source)
            .expect("Star vertex shader compile error");
        let star_fragment_shader = compile_shader(&gl, GL::FRAGMENT_SHADER, star_fragment_shader_source)
            .expect("Star fragment shader compile error");
        let star_program = link_program(&gl, &star_vertex_shader, &star_fragment_shader)
            .expect("Star program link error");

        let meteor_vertex_shader_source = r#"
            attribute vec2 a_position;
            attribute float a_alpha;
            attribute vec3 a_color;
            uniform vec2 u_resolution;
            varying float v_alpha;
            varying vec3 v_color;
            void main() {
                vec2 zeroToOne = a_position / u_resolution;
                vec2 zeroToTwo = zeroToOne * 2.0;
                vec2 clipSpace = zeroToTwo - 1.0;
                clipSpace.y = -clipSpace.y;
                gl_Position = vec4(clipSpace, 0.0, 1.0);
                v_alpha = a_alpha;
                v_color = a_color;
            }
        "#;
        let meteor_fragment_shader_source = r#"
            precision mediump float;
            varying float v_alpha;
            varying vec3 v_color;
            void main() {
                float dist = length(gl_PointCoord - vec2(0.5));
                float factor = smoothstep(0.5, 0.0, dist);
                gl_FragColor = vec4(v_color, v_alpha * factor);
            }
        "#;
        let meteor_vertex_shader = compile_shader(&gl, GL::VERTEX_SHADER, meteor_vertex_shader_source)
            .expect("Meteor vertex shader compile error");
        let meteor_fragment_shader = compile_shader(&gl, GL::FRAGMENT_SHADER, meteor_fragment_shader_source)
            .expect("Meteor fragment shader compile error");
        let meteor_program = link_program(&gl, &meteor_vertex_shader, &meteor_fragment_shader)
            .expect("Meteor program link error");

        let bottom_color = [54.0/255.0, 69.0/255.0, 125.0/255.0];
        let top_color = [25.0/255.0, 45.0/255.0, 105.0/255.0];
        let background_vertices: [f32; 6 * 5] = [
            -1.0, -1.0, bottom_color[0], bottom_color[1], bottom_color[2],
             1.0, -1.0, bottom_color[0], bottom_color[1], bottom_color[2],
            -1.0,  1.0, top_color[0],    top_color[1],    top_color[2],
             1.0, -1.0, bottom_color[0], bottom_color[1], bottom_color[2],
             1.0,  1.0, top_color[0],    top_color[1],    top_color[2],
            -1.0,  1.0, top_color[0],    top_color[1],    top_color[2],
        ];
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&background_buffer));
        unsafe {
            let vert_array = js_sys::Float32Array::view(&background_vertices);
            gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &vert_array, GL::STATIC_DRAW);
        }

        StarField {
            gl,
            canvas,
            stars,
            star_buffer,
            resolution,
            background_program,
            star_program,
            background_buffer,
            meteors,
            meteor_buffer,
            meteor_program,
        }
    }

    fn init_stars(stars: &mut Vec<Star>, num_stars: usize, width: f32, height: f32) {
        let center_x = width / 2.0;
        let center_y = height / 2.0;
        for _ in 0..num_stars {
            let mut x: f32;
            let mut y: f32;
            let r = js_sys::Math::random() as f32;
            let radius = 0.005 + (0.04 - 0.005) * r * r;
            
            if radius > 0.035 && (js_sys::Math::random() as f32) < 0.5 {
                x = center_x + ((js_sys::Math::random() as f32) - 0.5) * (width * 0.2);
                y = center_y + ((js_sys::Math::random() as f32) - 0.5) * (height * 0.2);
            } else {
                x = js_sys::Math::random() as f32 * width;
                let chance = js_sys::Math::random() as f32;
                if chance < 0.8 {
                    let u1 = (js_sys::Math::random() as f32).max(0.000001);
                    let u2 = js_sys::Math::random() as f32;
                    let gaussian = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos();
                    let sigma = height * 0.15;
                    y = center_y + sigma * gaussian;
                    y = y.max(0.0).min(height);
                } else {
                    y = js_sys::Math::random() as f32 * height;
                }
            }
            let vx = (js_sys::Math::random() as f32 - 0.5) * 0.1;
            let vy = (js_sys::Math::random() as f32 - 0.5) * 0.1;
            let r_val = js_sys::Math::random() as f32;
            let base_alpha = if r_val < 0.33 { 0.5 } else if r_val < 0.66 { 0.7 } else { 0.9 };
            let twinkle_phase = (js_sys::Math::random() as f32) * 6.28318530718;
            let twinkle_speed = 0.002 + (js_sys::Math::random() as f32) * 0.003;
            let choice = js_sys::Math::random() as f32;
            let color = if choice < 0.33 {
                [1.0, 0.8, 0.5]
            } else if choice < 0.66 {
                [0.5, 0.8, 1.0]
            } else {
                [1.0, 1.0, 1.0]
            };
            stars.push(Star {
                x,
                y,
                radius,
                vx,
                vy,
                base_alpha,
                twinkle_phase,
                twinkle_speed,
                alpha: base_alpha,
                color,
            });
        }
    }

    fn resize(&mut self) {
        let dpr = window().unwrap().device_pixel_ratio() as f32;
        let css_width = self.canvas.client_width() as f32;
        let css_height = self.canvas.client_height() as f32;
        let new_width = css_width * dpr;
        let new_height = css_height * dpr;
        
        let (old_width, old_height) = self.resolution;
        
        self.canvas.set_width(new_width as u32);
        self.canvas.set_height(new_height as u32);
        self.resolution = (new_width, new_height);

        if old_width <= 0.0 || old_height <= 0.0 {
            return;
        }

        self.stars.retain(|star| star.x >= 0.0 && star.x <= new_width &&
                           star.y >= 0.0 && star.y <= new_height);

        let old_area = old_width * old_height;
        let new_area = new_width * new_height;
        if new_area > old_area {
            let density = self.stars.len() as f32 / old_area.max(1.0);
            let extra_area = new_area - old_area;
            let stars_to_add = (density * extra_area).ceil() as usize;

            for _ in 0..stars_to_add {
                let (nx, ny) = pick_random_in_diff_area(old_width, old_height, new_width, new_height);
                
                let r = js_sys::Math::random() as f32;
                let radius = 0.005 + (0.04 - 0.005) * r * r;
                let vx = (js_sys::Math::random() as f32 - 0.5) * 0.1;
                let vy = (js_sys::Math::random() as f32 - 0.5) * 0.1;
                let r_val = js_sys::Math::random() as f32;
                let base_alpha = if r_val < 0.33 { 0.5 } else if r_val < 0.66 { 0.7 } else { 0.9 };
                let twinkle_phase = (js_sys::Math::random() as f32) * 6.28318530718;
                let twinkle_speed = 0.002 + (js_sys::Math::random() as f32) * 0.003;
                let choice = js_sys::Math::random() as f32;
                let color = if choice < 0.33 {
                    [1.0, 0.8, 0.5]
                } else if choice < 0.66 {
                    [0.5, 0.8, 1.0]
                } else {
                    [1.0, 1.0, 1.0]
                };
                self.stars.push(Star {
                    x: nx,
                    y: ny,
                    radius,
                    vx,
                    vy,
                    base_alpha,
                    twinkle_phase,
                    twinkle_speed,
                    alpha: base_alpha,
                    color,
                });
            }
        }
    }

    fn update(&mut self) {
        let dt: f32 = 1.0;
        const AMPLITUDE: f32 = 0.3;
        for star in &mut self.stars {
            star.x += star.vx * dt;
            star.y += star.vy * dt;
            star.vx *= 0.995;
            star.vy *= 0.995;
            if star.x > self.resolution.0 { star.x = 0.0; }
            if star.x < 0.0 { star.x = self.resolution.0; }
            if star.y > self.resolution.1 { star.y = 0.0; }
            if star.y < 0.0 { star.y = self.resolution.1; }
            star.twinkle_phase += star.twinkle_speed * dt;
            star.alpha = star.base_alpha + AMPLITUDE * star.twinkle_phase.sin();
            star.alpha = star.alpha.max(0.0).min(1.0);
        }
        const POINT_SCALE: f32 = 100.0;
        let mut star_data = Vec::with_capacity(self.stars.len() * 7);
        for star in &self.stars {
            let point_size = (star.radius * POINT_SCALE).max(1.0);
            star_data.push(star.x);
            star_data.push(star.y);
            star_data.push(point_size);
            star_data.push(star.alpha);
            star_data.push(star.color[0]);
            star_data.push(star.color[1]);
            star_data.push(star.color[2]);
        }
        self.gl.bind_buffer(GL::ARRAY_BUFFER, Some(&self.star_buffer));
        unsafe {
            let star_array = js_sys::Float32Array::view(&star_data);
            self.gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &star_array, GL::DYNAMIC_DRAW);
        }
        
        if (js_sys::Math::random() as f32) < 0.001 {
            let x = (js_sys::Math::random() as f32) * self.resolution.0;
            let y = (js_sys::Math::random() as f32) * self.resolution.1;
            let speed = 1.0;
            let angle = std::f32::consts::PI / 4.0;
            let vx = speed * angle.cos();
            let vy = speed * angle.sin();
            let max_lifetime = 50.0;
            let point_size = 0.0;
            let color = [1.0, 1.0, 0.8];
            self.meteors.push(Meteor {
                x, y, vx, vy,
                lifetime: 0.0,
                max_lifetime,
                point_size,
                color,
            });
        }
        for meteor in &mut self.meteors {
            meteor.x += meteor.vx * dt;
            meteor.y += meteor.vy * dt;
            meteor.lifetime += dt;
        }
        self.meteors.retain(|meteor| meteor.lifetime < meteor.max_lifetime);
        
        let mut meteor_data = Vec::new();
        for meteor in &self.meteors {
            let head_x = meteor.x;
            let head_y = meteor.y;
            let speed = (meteor.vx * meteor.vx + meteor.vy * meteor.vy).sqrt();
            let (norm_vx, norm_vy) = if speed > 0.0001 {
                (meteor.vx / speed, meteor.vy / speed)
            } else {
                (1.0, 0.0)
            };
            let tail_x = head_x - norm_vx * METEOR_TRAIL_LENGTH;
            let tail_y = head_y - norm_vy * METEOR_TRAIL_LENGTH;
            let perp_x = -norm_vy;
            let perp_y = norm_vx;
            let half_width = METEOR_WIDTH / 2.0;
            let v0x = head_x + perp_x * half_width;
            let v0y = head_y + perp_y * half_width;
            let v1x = head_x - perp_x * half_width;
            let v1y = head_y - perp_y * half_width;
            let v2x = tail_x + perp_x * half_width;
            let v2y = tail_y + perp_y * half_width;
            let v3x = tail_x - perp_x * half_width;
            let v3y = tail_y - perp_y * half_width;
            let base = 1.0 - (meteor.lifetime / meteor.max_lifetime);
            let head_alpha = base;
            let tail_alpha = 0.0;
            meteor_data.push(v0x);
            meteor_data.push(v0y);
            meteor_data.push(head_alpha);
            meteor_data.push(meteor.color[0]);
            meteor_data.push(meteor.color[1]);
            meteor_data.push(meteor.color[2]);
            
            meteor_data.push(v1x);
            meteor_data.push(v1y);
            meteor_data.push(head_alpha);
            meteor_data.push(meteor.color[0]);
            meteor_data.push(meteor.color[1]);
            meteor_data.push(meteor.color[2]);
            
            meteor_data.push(v2x);
            meteor_data.push(v2y);
            meteor_data.push(tail_alpha);
            meteor_data.push(meteor.color[0]);
            meteor_data.push(meteor.color[1]);
            meteor_data.push(meteor.color[2]);
            
            meteor_data.push(v1x);
            meteor_data.push(v1y);
            meteor_data.push(head_alpha);
            meteor_data.push(meteor.color[0]);
            meteor_data.push(meteor.color[1]);
            meteor_data.push(meteor.color[2]);
            
            meteor_data.push(v2x);
            meteor_data.push(v2y);
            meteor_data.push(tail_alpha);
            meteor_data.push(meteor.color[0]);
            meteor_data.push(meteor.color[1]);
            meteor_data.push(meteor.color[2]);
            
            meteor_data.push(v3x);
            meteor_data.push(v3y);
            meteor_data.push(tail_alpha);
            meteor_data.push(meteor.color[0]);
            meteor_data.push(meteor.color[1]);
            meteor_data.push(meteor.color[2]);
        }
        self.gl.bind_buffer(GL::ARRAY_BUFFER, Some(&self.meteor_buffer));
        unsafe {
            let meteor_array = js_sys::Float32Array::view(&meteor_data);
            self.gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &meteor_array, GL::DYNAMIC_DRAW);
        }
    }

    fn draw(&self) {
        let gl = &self.gl;
        gl.viewport(0, 0, self.resolution.0 as i32, self.resolution.1 as i32);
        gl.clear_color(0.0, 0.0, 0.0, 1.0);
        gl.clear(GL::COLOR_BUFFER_BIT);
        
        gl.use_program(Some(&self.background_program));
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&self.background_buffer));
        let pos_attrib_location = gl.get_attrib_location(&self.background_program, "a_position") as u32;
        let color_attrib_location = gl.get_attrib_location(&self.background_program, "a_color") as u32;
        let stride = 5 * std::mem::size_of::<f32>() as i32;
        gl.enable_vertex_attrib_array(pos_attrib_location);
        gl.vertex_attrib_pointer_with_i32(pos_attrib_location, 2, GL::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(color_attrib_location);
        gl.vertex_attrib_pointer_with_i32(
            color_attrib_location, 3, GL::FLOAT, false, stride, 2 * std::mem::size_of::<f32>() as i32
        );
        gl.draw_arrays(GL::TRIANGLES, 0, 6);
        
        gl.use_program(Some(&self.star_program));
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&self.star_buffer));
        let star_stride = 7 * std::mem::size_of::<f32>() as i32;
        let star_pos_loc = gl.get_attrib_location(&self.star_program, "a_position") as u32;
        let point_size_loc = gl.get_attrib_location(&self.star_program, "a_pointSize") as u32;
        let alpha_loc = gl.get_attrib_location(&self.star_program, "a_alpha") as u32;
        let color_loc = gl.get_attrib_location(&self.star_program, "a_color") as u32;
        gl.enable_vertex_attrib_array(star_pos_loc);
        gl.vertex_attrib_pointer_with_i32(star_pos_loc, 2, GL::FLOAT, false, star_stride, 0);
        gl.enable_vertex_attrib_array(point_size_loc);
        gl.vertex_attrib_pointer_with_i32(point_size_loc, 1, GL::FLOAT, false, star_stride, 2 * std::mem::size_of::<f32>() as i32);
        gl.enable_vertex_attrib_array(alpha_loc);
        gl.vertex_attrib_pointer_with_i32(alpha_loc, 1, GL::FLOAT, false, star_stride, 3 * std::mem::size_of::<f32>() as i32);
        gl.enable_vertex_attrib_array(color_loc);
        gl.vertex_attrib_pointer_with_i32(color_loc, 3, GL::FLOAT, false, star_stride, 4 * std::mem::size_of::<f32>() as i32);
        if let Some(loc) = gl.get_uniform_location(&self.star_program, "u_resolution") {
            gl.uniform2f(Some(&loc), self.resolution.0, self.resolution.1);
        }
        gl.draw_arrays(GL::POINTS, 0, self.stars.len() as i32);
        
        gl.use_program(Some(&self.meteor_program));
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&self.meteor_buffer));
        let meteor_stride = 6 * std::mem::size_of::<f32>() as i32; // (x,y,alpha,r,g,b)
        let meteor_pos_loc = gl.get_attrib_location(&self.meteor_program, "a_position") as u32;
        let meteor_alpha_loc = gl.get_attrib_location(&self.meteor_program, "a_alpha") as u32;
        let meteor_color_loc = gl.get_attrib_location(&self.meteor_program, "a_color") as u32;
        gl.enable_vertex_attrib_array(meteor_pos_loc);
        gl.vertex_attrib_pointer_with_i32(meteor_pos_loc, 2, GL::FLOAT, false, meteor_stride, 0);
        gl.enable_vertex_attrib_array(meteor_alpha_loc);
        gl.vertex_attrib_pointer_with_i32(meteor_alpha_loc, 1, GL::FLOAT, false, meteor_stride, 2 * std::mem::size_of::<f32>() as i32);
        gl.enable_vertex_attrib_array(meteor_color_loc);
        gl.vertex_attrib_pointer_with_i32(meteor_color_loc, 3, GL::FLOAT, false, meteor_stride, 3 * std::mem::size_of::<f32>() as i32);
        if let Some(loc) = gl.get_uniform_location(&self.meteor_program, "u_resolution") {
            gl.uniform2f(Some(&loc), self.resolution.0, self.resolution.1);
        }
        gl.draw_arrays(GL::TRIANGLES, 0, (self.meteors.len() * 6) as i32);
    }
}

fn pick_random_in_diff_area(old_width: f32, old_height: f32, new_width: f32, new_height: f32) -> (f32, f32) {
    if new_width <= old_width && new_height <= old_height {
        return (js_sys::Math::random() as f32 * new_width,
                js_sys::Math::random() as f32 * new_height);
    }
    loop {
        let x = js_sys::Math::random() as f32 * new_width;
        let y = js_sys::Math::random() as f32 * new_height;
        if x > old_width || y > old_height {
            return (x, y);
        }
    }
}

#[wasm_bindgen]
pub fn start_starfield(canvas_id: &str, num_stars: usize) {
    let star_field = Rc::new(RefCell::new(StarField::new(canvas_id, num_stars)));
    
    {
        let star_field_clone = star_field.clone();
        let resize_closure = Closure::wrap(Box::new(move || {
            star_field_clone.borrow_mut().resize();
        }) as Box<dyn FnMut()>);
        window().unwrap()
            .add_event_listener_with_callback("resize", resize_closure.as_ref().unchecked_ref())
            .unwrap();
        resize_closure.forget();
    }
    
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();
    
    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        {
            let mut sf = star_field.borrow_mut();
            sf.update();
            sf.draw();
        }
        window().unwrap()
            .request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
            .unwrap();
    }) as Box<dyn FnMut()>));
    
    window().unwrap()
        .request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref())
        .unwrap();
}

fn compile_shader(gl: &GL, shader_type: u32, source: &str) -> Result<WebGlShader, String> {
    let shader = gl.create_shader(shader_type).ok_or("Unable to create shader object")?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);
    if gl.get_shader_parameter(&shader, GL::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(gl.get_shader_info_log(&shader).unwrap_or_else(|| "Unknown error creating shader".into()))
    }
}

fn link_program(gl: &GL, vertex_shader: &WebGlShader, fragment_shader: &WebGlShader) -> Result<WebGlProgram, String> {
    let program = gl.create_program().ok_or("Unable to create shader program")?;
    gl.attach_shader(&program, vertex_shader);
    gl.attach_shader(&program, fragment_shader);
    gl.link_program(&program);
    if gl.get_program_parameter(&program, GL::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(gl.get_program_info_log(&program).unwrap_or_else(|| "Unknown error linking program".into()))
    }
}
