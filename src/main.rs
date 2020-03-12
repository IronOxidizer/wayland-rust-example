extern crate wayland_client;
use wayland_client::{Display, GlobalManager};
use wayland_client::protocol::{wl_compositor, wl_shm};
use wayland_protocols::xdg_shell::client::{xdg_wm_base, xdg_surface};

extern crate byteorder;
extern crate tempfile;
use std::cmp::min;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use byteorder::{NativeEndian, WriteBytesExt};

fn main() {
    // Initialize env, event_queue, and registry (GlobalManager)
    let display = Display::connect_to_env().expect("Failed to connect to wayland server");
    let mut event_queue = display.create_event_queue();
    let attached_display = display.attach(event_queue.token());
    let global_manager = GlobalManager::new(&attached_display);

    // Sync environement globals
    event_queue
        .sync_roundtrip(&mut (), |_, _, _| unreachable!())
        .unwrap();

    // Define compositor and create a surface
    let compositor = global_manager
        .instantiate_exact::<wl_compositor::WlCompositor>(1)
        .unwrap();
    let surface = compositor.create_surface();

    // TODO:
    // - Make this much simpler with less dependencies
    // Create a gradient and store in a buffer for later rendering on the surface
    let buf_x: u32 = 320;
    let buf_y: u32 = 240;
    let mut tmp = tempfile::tempfile().ok().expect("Unable to create a tempfile.");
    for i in 0..(buf_x * buf_y) {
        let x = (i % buf_x) as u32;
        let y = (i / buf_x) as u32;
        let r: u32 = min(((buf_x - x) * 0xFF) / buf_x, ((buf_y - y) * 0xFF) / buf_y);
        let g: u32 = min((x * 0xFF) / buf_x, ((buf_y - y) * 0xFF) / buf_y);
        let b: u32 = min(((buf_x - x) * 0xFF) / buf_x, (y * 0xFF) / buf_y);
        let _ = tmp.write_u32::<NativeEndian>((0xFF << 24) + (r << 16) + (g << 8) + b);
    }
    let _ = tmp.flush();
    let shm = global_manager.instantiate_exact::<wl_shm::WlShm>(1).unwrap();
    let pool = shm.create_pool(
        tmp.as_raw_fd(),            // RawFd to the tempfile serving as shared memory
        (buf_x * buf_y * 4) as i32, // size in bytes of the shared memory (4 bytes per pixel)
    );
    let buffer = pool.create_buffer(
        0,                        // Start of the buffer in the pool
        buf_x as i32,             // width of the buffer in pixels
        buf_y as i32,             // height of the buffer in pixels
        (buf_x * 4) as i32,       // number of bytes between the beginning of two consecutive lines
        wl_shm::Format::Argb8888, // chosen encoding for the data
    );

    // Create shell and assign surface to shell
    let shell = global_manager
        .instantiate_exact::<xdg_wm_base::XdgWmBase>(1)
        .expect("Could not create shell");
    let shell_surface = shell.get_xdg_surface(&surface);
    shell_surface.get_toplevel();
    surface.commit();

    // Configure shell surface
    shell_surface.quick_assign(move |shell_surface, event, _| match event {
        xdg_surface::Event::Configure { serial } => {
            shell_surface.ack_configure(serial);

            // Set our surface as top level and define its contents
            surface.attach(Some(&buffer), 0, 0);
            surface.commit();
        }
        _ => unreachable!(),
    });

    // Sync wl globals
    event_queue
        .sync_roundtrip(&mut (), |_, _, _| { /* we ignore unfiltered messages */ })
        .unwrap();

    // Poll for events
    loop {
        event_queue
            .dispatch(&mut (), |_, _, _| { /* we ignore unfiltered messages */ })
            .unwrap();
    }
}
