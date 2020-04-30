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
use ifaces::*;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::net::SocketAddr;

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
    for iface in ifaces::Interface::get_all().unwrap().into_iter() {
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

fn main() {
    let application = Application::new(
        Some("com.github.arjo129.remote.tablet"),
        Default::default(),
    ).expect("failed to initialize GTK application");

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
