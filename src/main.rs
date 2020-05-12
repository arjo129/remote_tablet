extern crate gtk;
extern crate gio;

use gtk::prelude::*;
use gio::prelude::*;
use gdk_pixbuf::prelude::*;
use gdk_pixbuf::{Pixbuf, PixbufLoader};
use gdk_pixbuf::Colorspace;
use image::{Rgb, ImageOutputFormat, DynamicImage};
use qrcode::QrCode;
use gtk::{Application, ApplicationWindow, Button, Image};

use ifaces::Interface;

use tiny_http::{Server, Response};

use websocket::sync::Server as WsServer;
use websocket::OwnedMessage;

use serde_json::Value;

use enigo::*;

use std::time::{Duration, SystemTime};
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::net::SocketAddr;
use std::thread;

#[derive(Debug, Copy, Clone)]
enum IfaceError {
    FailedToReadProcFs,
    UnableToGetIpForWiFi,
    NoNetworkIface
}

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

fn get_wireless_interface()->Result<Vec<String>, IfaceError> {
    let mut iface_names = vec!();
    if let Ok(lines) = read_lines("/proc/net/wireless"){
        let mut i = 0;
        for line in lines {
            if i > 0 {
                match line {
                    Ok(line) => {
                        let split: Vec<&str> = line.split(":").collect();
                        if split.len() > 1 {
                            iface_names.push(split[0].to_string());
                        }
                    },
                    Err(_)=>{}
                }
            }
            i += 1;
        }
        return Ok(iface_names);
    }
    else {
        return Err(IfaceError::FailedToReadProcFs);
    }
}

fn get_ip_address_given_iface_name(iface_name: String) -> Option<SocketAddr>{
    let mut res: Option<SocketAddr> = None;
    for iface in Interface::get_all().unwrap().into_iter() {
        if iface.name == iface_name {
            if let Some(SocketAddr::V4(ipaddr)) = iface.addr {res = iface.addr;}
        }
    }
    res
}

fn get_ip() ->  Result<String, IfaceError> {
    let wifi_interfaces = get_wireless_interface();
    match wifi_interfaces {
        Ok(interfaces) => {
            for i in interfaces {
                let address = get_ip_address_given_iface_name(i);
                match address {
                    Some(addr) => return Ok(addr.ip().to_string()),
                    None => return Err(IfaceError::UnableToGetIpForWiFi)
                }
            }
            return Err(IfaceError::NoNetworkIface);
        },
        Err(e) => {
            return Err(e);
        }
    }
}

struct MousePosition {
    x: f64,
    y: f64,
    stylus: bool
}

enum MouseEvent {
    MouseMove(MousePosition),
    MouseRelease
}

fn deseriallize_data(data: OwnedMessage) -> Option<MouseEvent> {
    
    if let OwnedMessage::Text(payload) = data {
        if let Ok(z)  = serde_json::from_str(&payload) {
            let v:Value = z;
            if let Some(event_type) =  v["type"].as_str() {
                if event_type == "end" {
                    return Some(MouseEvent::MouseRelease);
                }
            }
            else {
                return None;
            }
            println!("{} {} {} {}", v["x"], v["y"], v["force"], v["touch_type"]);
            let x = v["x"].as_f64();
            let y = v["y"].as_f64();
            let touch_type = v["touch_type"].as_str();
            if let (Some(x), Some(y), Some(touch_type)) = (x,  y, touch_type) {
                return Some(MouseEvent::MouseMove(MousePosition{ x:x, y:y, stylus: touch_type == "stylus"}));    
            } 
        }
        else {
            println!("Client sent weird data");
        }
    }
    None        
}

fn handle_event(enigo: &mut Enigo, mouse_position: MouseEvent) {
    match mouse_position {
        MouseEvent::MouseMove(mouse_position) => {
            enigo.mouse_move_to(mouse_position.x as i32, mouse_position.y as i32);
            enigo.mouse_down(MouseButton::Left);
        },

        MouseEvent::MouseRelease => {
            enigo.mouse_up(MouseButton::Left);
        }
    }   
}
fn run_websocket_server() {
    let server = WsServer::bind("0.0.0.0:2794").expect("Could not open websocket");
    let mut enigo = Enigo::new();
    let mut last_time = SystemTime::now();
    thread::spawn(move || {
        println!("running web sockets");
        for request in server.filter_map(Result::ok) {
            if !request.protocols().contains(&"rust-websocket".to_string()) {
                println!("Rejected client");
				request.reject().unwrap();
				return;
			}

			let mut client = request.use_protocol("rust-websocket").accept().unwrap();

            let ip = client.peer_addr().unwrap();
            println!("Connection from {}", ip);

            let (mut receiver, mut sender) = client.split().unwrap();
            
            for message in receiver.incoming_messages() {
                match message {
                    Ok(x)=> {
                        if let Some(mouse_position) = deseriallize_data(x){
                            handle_event(&mut enigo, mouse_position);
                        }
                    },
                    Err(e)=> break
                }

            }
        }
    });
}

fn run_http_server() {
    let http_file: Vec<u8> = include_bytes!("../html/index.html").iter().cloned().collect();
    let server = Server::http("0.0.0.0:8007").expect("Failed to start HTTP Server");
    thread::spawn(move || {
        loop {
            let request = match server.recv() {
                Ok(rq) => rq,
                Err(e) => { println!("error: {}", e); continue }
            };
            let resp = Response::from_data(http_file.clone());
            request.respond(resp);
        }
    });
}

fn main() {
    let application = Application::new(
        Some("com.github.arjo129.remote.tablet"),
        Default::default(),
    ).expect("failed to initialize GTK application");

    run_http_server();
    run_websocket_server();

    application.connect_activate(|app| {
        let window = ApplicationWindow::new(app);
        window.set_title("Remote Tablet Program");
        window.set_default_size(350, 350);
        let ip = get_ip();
        match ip.clone() {
            Ok(_) =>{}
            Err(e) => {println!("Error fetching IP {:?}", e); return;}
        }
        let ip = format!("http://{}:8007", ip.unwrap());
        println!("{}", ip);
        let code = QrCode::new(ip).unwrap();
        let image = DynamicImage::ImageRgb8(code.render::<Rgb<u8>>().build());
        let mut buff = Vec::<u8>::new(); 
        let res = image.write_to(&mut buff, ImageOutputFormat::Png);
        match (res) {
            Ok(_) => {
                let loader = PixbufLoader::new();
                let res = loader.write(&buff);
                loader.connect_area_prepared(move |loader| {
                    let pixbuf = loader.get_pixbuf();
                    let image_view = Image::new_from_pixbuf(pixbuf.as_ref());
                    window.add(&image_view);
                    window.show_all();
                    println!("Generated qr code");
                });
                loader.close();
            },
            Err(_) => {
                println!("Error generating QR code")
            }
        }
    });


    application.run(&[]);
}
