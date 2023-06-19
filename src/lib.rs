use detour::static_detour;
use std::{mem, ptr};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use lazy_static::lazy_static;
use winapi::um::libloaderapi::GetModuleHandleA;

unsafe fn get_function_ptr(offset: i32) -> *const u8 {
    let module_handle = GetModuleHandleA(ptr::null()) as usize;
    (module_handle + (offset as usize)) as *const u8
}

#[repr(C)]
struct Level {
    size_fill: [u8; 0x32f0],
}

lazy_static! {
    static ref TICK_VEC: Mutex<Option<Vec<u128>>> = Mutex::new(None);
}

static_detour! {
    static level_tick_hook: unsafe extern "C" fn(*mut Level);
}

type LevelTick = extern "C" fn(*mut Level);

#[no_mangle]
pub extern "C" fn init(level_tick_offset: i32) -> i32 {
    let level_tick_func: LevelTick = unsafe { mem::transmute(get_function_ptr(level_tick_offset))};

    let init_result = unsafe { level_tick_hook.initialize(level_tick_func, on_tick) };
    if init_result.is_err() {
        return 1;
    }

    let enable_result = unsafe { init_result.unwrap().enable() };
    if enable_result.is_err() {
        return 1;
    }

    let mut guard = TICK_VEC.lock().unwrap();
    *guard = Some(Vec::new());

    drop(guard);

    0
}

#[no_mangle]
pub extern "C" fn get_tps() -> i32 {
    // Grabbing the vector
    let guard_t = TICK_VEC.lock();
    let mut guard = guard_t.unwrap();
    let vec_option = guard.as_mut();

    if vec_option.is_none() {
        return -1;
    }

    let tick_vec = vec_option.unwrap();

    let mut combined_tick_duration: u128 = 0;

    let mut last_time: Option<u128> = None;
    for check_time in tick_vec.iter_mut() {
        if last_time.is_none() {
            last_time = Some(*check_time);
            continue;
        }

        combined_tick_duration += *check_time - last_time.unwrap();
        last_time = Some(*check_time);
    }

    let avg_tick_duration = combined_tick_duration / (tick_vec.len() as u128);

    (1000 / avg_tick_duration) as i32
}

fn on_tick(level: *mut Level) {
    let tick_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Failed to get time since EPOCH!")
        .as_millis();

    let mut guard = TICK_VEC.lock().unwrap();
    let vec_option = guard.as_mut();
    let tick_vec = vec_option.unwrap();
    tick_vec.push(tick_time);

    // Checking that tick_vec doesn't have more than 200 members, removing oldest if it does
    if tick_vec.len() > 200 {
        let values_to_remove = tick_vec.len() - 200;
        for _ in 0..values_to_remove {
            tick_vec.remove(0);
        }
    }

    drop(guard);

    unsafe { level_tick_hook.call(level) }
}