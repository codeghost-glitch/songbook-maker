//! JNI entry point for the Android app.
//!
//! Kotlin side (Engine.kt):
//!   external fun nativeRender(input: String, config: String): ByteArray
//! The return value is the packet built by crate::packet.

use jni::objects::{JClass, JString};
use jni::sys::jbyteArray;
use jni::JNIEnv;

fn read(env: &mut JNIEnv, s: &JString) -> Option<String> {
    let js = env.get_string(s).ok()?;
    js.to_str().ok().map(|c| c.to_owned())
}

#[no_mangle]
pub extern "system" fn Java_io_github_codeghost_songbook_Engine_nativeRender(
    mut env: JNIEnv,
    _class: JClass,
    input: JString,
    config: JString,
) -> jbyteArray {
    let text = match read(&mut env, &input) {
        Some(t) => t,
        None => return std::ptr::null_mut(),
    };
    let cfg = match read(&mut env, &config) {
        Some(c) => c,
        None => return std::ptr::null_mut(),
    };
    let packet = crate::packet(crate::render_with_config(&text, &cfg));
    match env.byte_array_from_slice(&packet) {
        Ok(arr) => arr.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn to_jstring(env: &mut JNIEnv, s: &str) -> jni::sys::jstring {
    match env.new_string(s) {
        Ok(js) => js.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "system" fn Java_io_github_codeghost_songbook_Engine_nativeInspect(
    mut env: JNIEnv,
    _class: JClass,
    input: JString,
    order: JString,
) -> jni::sys::jstring {
    let text = match read(&mut env, &input) {
        Some(t) => t,
        None => return std::ptr::null_mut(),
    };
    let ord = match read(&mut env, &order) {
        Some(o) => o,
        None => return std::ptr::null_mut(),
    };
    let order = match crate::import::Order::parse(&ord) {
        Some(o) => o,
        None => return std::ptr::null_mut(),
    };
    let meta = crate::inspect_meta(&text, order);
    to_jstring(&mut env, &meta)
}
