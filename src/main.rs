#[macro_use]
extern crate log;

use core::slice;
use naga::{
    back, front,
    valid::{ValidationFlags, Validator},
};
use std::{io, mem::size_of};
use tokio::{fs::File, io::AsyncWriteExt};

const TEMPLATE_SOURCE: &str = include_str!("template.wgsl");

#[tokio::main(flavor = "current_thread")]
async fn main() {
    dotenv::dotenv().ok();
    env_logger::init();

    let _spirv = load_shaders().await;
}

async fn load_shaders() -> Vec<u32> {
    info!("Loading utility functions...");
    let module = front::wgsl::parse_str(TEMPLATE_SOURCE).unwrap();

    info!("Validating module...");
    let mut validator = Validator::new(ValidationFlags::all());
    let module_info = validator.validate(&module).unwrap();

    info!("Writing module as txt...");
    let mut file = File::create("debug.txt").await.unwrap();
    file.write_all(format!("{:#?}", &module).as_bytes())
        .await
        .unwrap();

    info!("Compiling spirv...");
    let mut spirv = vec![];
    let options = back::spv::Options::default();
    let mut writer = back::spv::Writer::new(&options).unwrap();
    writer.write(&module, &module_info, &mut spirv).unwrap();

    info!("Writing spirv...");
    let mut spirv_file = File::create("debug.spv").await.unwrap();
    write_spirv(&mut spirv_file, &spirv).await.unwrap();

    spirv
}

async fn write_spirv(writer: &mut File, spirv: &[u32]) -> io::Result<()> {
    let u8slice = unsafe {
        slice::from_raw_parts(spirv.as_ptr() as *const u8, spirv.len() * size_of::<u32>())
    };
    writer.write_all(u8slice).await
}
