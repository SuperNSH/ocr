//! Android: ML Kit text recognition v2, reached over JNI.
//!
//! The VM comes from `ndk-context`; this module installs it once with the pattern
//! `nsh-biometrics` uses, because NSH does not start `ndk-context` itself. The app's APK must
//! carry the ML Kit v2 dependencies (`com.google.mlkit:text-recognition` 16.0.1 plus the
//! `-chinese`, `-devanagari`, `-japanese` and `-korean` artifacts) and this bridge class,
//! which turns ML Kit's task listeners into native calls:
//!
//! ```java
//! package dev.supernsh.ocr;
//!
//! import com.google.android.gms.tasks.OnFailureListener;
//! import com.google.android.gms.tasks.OnSuccessListener;
//!
//! public final class NativeCallback implements OnSuccessListener<Object>, OnFailureListener {
//!     private long pointer;
//!
//!     public NativeCallback(long pointer) { this.pointer = pointer; }
//!
//!     private native void succeeded(long pointer, Object text);
//!     private native void failed(long pointer, Exception error);
//!
//!     @Override public synchronized void onSuccess(Object text) {
//!         if (pointer != 0) { long p = pointer; pointer = 0; succeeded(p, text); }
//!     }
//!
//!     @Override public synchronized void onFailure(Exception error) {
//!         if (pointer != 0) { long p = pointer; pointer = 0; failed(p, error); }
//!     }
//! }
//! ```
//!
//! NSH 0.4.0 writes the Android packaging itself and offers no way to add Java, so the app
//! adds the class to the generated project, the way `nsh-biometrics` documents adding the
//! biometric permission. Without those classes recognition answers `ERR_OCR_NOT_AVAILABLE`.

// jni 0.22.4 and ndk-context 0.1.1, the pins Cargo.toml declares for `target_os = "android"`.

use std::{
    collections::HashMap,
    ffi::{c_char, c_int, c_void},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
};

use jni::{
    Env, EnvUnowned, NativeMethod,
    objects::{JClass, JObject, JString, JValue},
    refs::Global,
    signature::{FieldSignature, MethodSignature, RuntimeFieldSignature, RuntimeMethodSignature},
    strings::JNIString,
    sys::{jbyte, jint, jlong, jsize},
    vm::JavaVM,
};

use crate::{
    backend::{Backend, BackendFuture, RecognitionDone},
    error::{ErrorKind, OcrError},
    input::ImageInput,
    options::{RecognitionOptions, Script},
    text::{Point, RecognizedText, Rect, TextBlock, TextElement, TextLine, TextSymbol},
};

pub(crate) struct NativeBackend;

impl From<jni::errors::Error> for OcrError {
    fn from(_: jni::errors::Error) -> Self {
        OcrError::new(ErrorKind::Backend)
    }
}

// `dlopen` and `dlsym` come from libdl, which the Android target links by default; the libc
// crate is not a dependency of this crate.
unsafe extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

/// bionic's `RTLD_NOW`.
const RTLD_NOW: c_int = 2;

/// `JNI_GetCreatedJavaVMs`, found by name in `libnativehelper`.
type GetCreatedVms = unsafe extern "C" fn(*mut jni::sys::JavaVM, jsize, *mut jsize) -> jint;

/// Set once `ndk-context` holds the VM and the application. It can be set only once.
static CONTEXT: Mutex<bool> = Mutex::new(false);
/// Guards the one `RegisterNatives` call for the bridge class.
static BRIDGE: Mutex<bool> = Mutex::new(false);
/// The calls waiting for their one listener answer, by the id the bridge object carries.
static CALLS: Mutex<HashMap<u64, Pending>> = Mutex::new(HashMap::new());
/// Hands out the id each recognition passes to its bridge object.
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// One started recognition: its answer and the ML Kit client that runs it.
struct Pending {
    done: RecognitionDone,
    client: Global<JObject<'static>>,
}

const TEXT_RECOGNITION: &str = "com/google/mlkit/vision/text/TextRecognition";
const INPUT_IMAGE: &str = "com/google/mlkit/vision/common/InputImage";
const BITMAP: &str = "android/graphics/Bitmap";
const BITMAP_FACTORY: &str = "android/graphics/BitmapFactory";
const BITMAP_CONFIG: &str = "android/graphics/Bitmap$Config";
const REFLECT_ARRAY: &str = "java/lang/reflect/Array";
const ACTIVITY_THREAD: &str = "android/app/ActivityThread";
const BRIDGE_CLASS: &str = "dev/supernsh/ocr/NativeCallback";
/// `Class.getName()` form, which is what a failed task's exception is compared against.
const MLKIT_EXCEPTION: &str = "com.google.mlkit.common.MlKitException";

const GET_CLIENT: &str = "(Lcom/google/mlkit/vision/text/TextRecognizerOptions;)Lcom/google/mlkit/vision/text/TextRecognizer;";
const PROCESS: &str =
    "(Lcom/google/mlkit/vision/common/InputImage;)Lcom/google/android/gms/tasks/Task;";
const ADD_ON_SUCCESS: &str =
    "(Lcom/google/android/gms/tasks/OnSuccessListener;)Lcom/google/android/gms/tasks/Task;";
const ADD_ON_FAILURE: &str =
    "(Lcom/google/android/gms/tasks/OnFailureListener;)Lcom/google/android/gms/tasks/Task;";
const DECODE_BYTE_ARRAY: &str = "([BII)Landroid/graphics/Bitmap;";
const CREATE_BITMAP: &str = "([IIILandroid/graphics/Bitmap$Config;)Landroid/graphics/Bitmap;";
const FROM_BITMAP: &str = "(Landroid/graphics/Bitmap;I)Lcom/google/mlkit/vision/common/InputImage;";
const CONFIG_FIELD: &str = "Landroid/graphics/Bitmap$Config;";
const STRING_RETURN: &str = "()Ljava/lang/String;";
const CLASS_RETURN: &str = "()Ljava/lang/Class;";
const GET_LIST: &str = "()Ljava/util/List;";
const GET_RECT: &str = "()Landroid/graphics/Rect;";
const GET_POINTS: &str = "()[Landroid/graphics/Point;";
const GET_BOXED_FLOAT: &str = "()Ljava/lang/Float;";
const FLOAT_VALUE: &str = "()F";
const INT_RETURN: &str = "()I";
const CLOSE: &str = "()V";
const BRIDGE_CTOR: &str = "(J)V";
const NATIVE_SUCCESS: &str = "(JLjava/lang/Object;)V";
const NATIVE_FAILURE: &str = "(JLjava/lang/Exception;)V";
const LIST_GET: &str = "(I)Ljava/lang/Object;";
const ARRAY_LENGTH: &str = "(Ljava/lang/Object;)I";
const ARRAY_GET: &str = "(Ljava/lang/Object;I)Ljava/lang/Object;";
const CURRENT_APPLICATION: &str = "()Landroid/app/Application;";
const INT_FIELD: &str = "I";

/// How to build the ML Kit options object for one script.
struct OptionsSpec {
    /// Every script's options class extends ML Kit's base `TextRecognizerOptions`, which is
    /// the type `TextRecognition.getClient` declares.
    class: &'static str,
    build: Build,
}

#[derive(Debug, PartialEq, Eq)]
enum Build {
    /// Read the class's `DEFAULT_OPTIONS` field.
    Default,
    /// Run `Builder().build()` on the class's `Builder`.
    Builder,
}

/// The options object ML Kit's `getClient` takes for one script.
fn options_for_script(script: Script) -> OptionsSpec {
    let (class, build) = match script {
        Script::Latin => (
            "com/google/mlkit/vision/text/latin/TextRecognizerOptions",
            Build::Default,
        ),
        Script::Chinese => (
            "com/google/mlkit/vision/text/chinese/ChineseTextRecognizerOptions",
            Build::Builder,
        ),
        Script::Devanagiri => (
            "com/google/mlkit/vision/text/devanagari/DevanagariTextRecognizerOptions",
            Build::Builder,
        ),
        Script::Japanese => (
            "com/google/mlkit/vision/text/japanese/JapaneseTextRecognizerOptions",
            Build::Builder,
        ),
        Script::Korean => (
            "com/google/mlkit/vision/text/korean/KoreanTextRecognizerOptions",
            Build::Builder,
        ),
    };
    OptionsSpec { class, build }
}

/// Packs RGBA8 bytes into the ARGB_8888 ints `Bitmap.createBitmap` takes.
fn rgba_to_argb(data: &[u8]) -> Vec<jint> {
    data.chunks_exact(4)
        .map(|px| {
            ((u32::from(px[3]) << 24)
                | (u32::from(px[0]) << 16)
                | (u32::from(px[1]) << 8)
                | u32::from(px[2])) as jint
        })
        .collect()
}

/// The five languages ML Kit v2 recognises, one per script.
fn supported() -> Vec<String> {
    [
        Script::Latin,
        Script::Chinese,
        Script::Devanagiri,
        Script::Japanese,
        Script::Korean,
    ]
    .into_iter()
    .map(|script| script.primary_language().to_owned())
    .collect()
}

/// A failed task's exception as one coded error; an ML Kit exception keeps its platform code.
fn map_task_failure(class: &str, code: Option<i32>) -> OcrError {
    if class == MLKIT_EXCEPTION
        && let Some(code) = code
    {
        return OcrError::new(ErrorKind::Backend).with_status(i64::from(code));
    }
    OcrError::new(ErrorKind::Backend)
}

/// Parses one of the method descriptors above; a malformed one is a coded failure.
fn method(descriptor: &str) -> Result<RuntimeMethodSignature, OcrError> {
    RuntimeMethodSignature::from_str(descriptor).map_err(OcrError::from)
}

/// Parses one of the field descriptors above; a malformed one is a coded failure.
fn field(descriptor: &str) -> Result<RuntimeFieldSignature, OcrError> {
    RuntimeFieldSignature::from_str(descriptor).map_err(OcrError::from)
}

/// The runtime's VM. An NSH app has no JVM of its own, so it is found through the runtime's library.
fn find_vm() -> Result<*mut jni::sys::JavaVM, OcrError> {
    // SAFETY: the names are valid C strings, and the symbol is called with the signature the
    // JNI specification gives it.
    unsafe {
        let library = dlopen(c"libnativehelper.so".as_ptr(), RTLD_NOW);
        if library.is_null() {
            return Err(OcrError::new(ErrorKind::NotAvailable));
        }
        let symbol = dlsym(library, c"JNI_GetCreatedJavaVMs".as_ptr());
        if symbol.is_null() {
            return Err(OcrError::new(ErrorKind::NotAvailable));
        }
        let get: GetCreatedVms = std::mem::transmute::<*mut c_void, GetCreatedVms>(symbol);
        let mut vm: *mut jni::sys::JavaVM = std::ptr::null_mut();
        let mut count: jsize = 0;
        if get(&mut vm, 1, &mut count) != 0 || count < 1 || vm.is_null() {
            return Err(OcrError::new(ErrorKind::NotAvailable));
        }
        Ok(vm)
    }
}

/// The application, which is the Android context `ndk-context` carries. Null before the runtime
/// has created it.
fn application<'local>(env: &mut Env<'local>) -> Result<JObject<'local>, OcrError> {
    let application = static_object(
        env,
        ACTIVITY_THREAD,
        "currentApplication",
        CURRENT_APPLICATION,
        &[],
    )?;
    if application.is_null() {
        return Err(OcrError::new(ErrorKind::NotAvailable));
    }
    Ok(application)
}

/// Gives `ndk-context` the VM and the application so this module can reach Java. A context
/// another runtime installed already is adopted untouched.
fn ensure_context() -> Result<(), OcrError> {
    let mut ready = CONTEXT.lock().unwrap_or_else(PoisonError::into_inner);
    if *ready {
        return Ok(());
    }
    // `ndk-context` panics while nothing is installed and asserts on a second initialization,
    // so a silenced probe is the only way to tell an installed context apart.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let probe = catch_unwind(AssertUnwindSafe(ndk_context::android_context));
    std::panic::set_hook(previous);
    if probe.is_ok() {
        *ready = true;
        return Ok(());
    }
    let vm = find_vm()?;
    // SAFETY: the pointer came from the runtime's own `JNI_GetCreatedJavaVMs`, and a VM lives
    // as long as the process.
    let context = unsafe { JavaVM::from_raw(vm) }.attach_current_thread(|env| {
        // The global reference lives as long as the process, which is what `ndk-context`
        // assumes.
        Ok::<_, OcrError>(
            env.new_global_ref(application(env)?)?
                .into_raw()
                .cast::<c_void>(),
        )
    })?;
    // SAFETY: the VM and the application are the process's own, and the probe showed that
    // nothing is installed yet.
    unsafe { ndk_context::initialize_android_context(vm.cast(), context) };
    *ready = true;
    Ok(())
}

/// The VM `ndk-context` carries. Only called after `ensure_context`, because the getter panics
/// while no context is installed.
fn vm() -> Result<JavaVM, OcrError> {
    // SAFETY: `ndk-context` was initialized with the runtime's own VM pointer, and a VM lives
    // as long as the process.
    Ok(unsafe { JavaVM::from_raw(ndk_context::android_context().vm().cast()) })
}

/// Binds the native listener methods to the bridge class, once. `RegisterNatives` is
/// idempotent, so a race only repeats the work.
fn register_bridge(env: &mut Env<'_>) -> Result<(), OcrError> {
    let mut ready = BRIDGE.lock().unwrap_or_else(PoisonError::into_inner);
    if *ready {
        return Ok(());
    }
    let class = env.find_class(JNIString::new(BRIDGE_CLASS))?;
    let succeeded_name = JNIString::new("succeeded");
    let succeeded_sig = JNIString::new(NATIVE_SUCCESS);
    let failed_name = JNIString::new("failed");
    let failed_sig = JNIString::new(NATIVE_FAILURE);
    let methods = [
        // SAFETY: both are instance methods - each function is (env, this, pointer, payload),
        // matching its descriptor - and the class is the one that declares them.
        unsafe {
            NativeMethod::from_raw_parts(&succeeded_name, &succeeded_sig, succeeded as *mut c_void)
        },
        // SAFETY: same shape as above, with the exception payload `failed` declares.
        unsafe { NativeMethod::from_raw_parts(&failed_name, &failed_sig, failed as *mut c_void) },
    ];
    // SAFETY: the descriptors state instance methods and the functions match them exactly.
    unsafe { env.register_native_methods(&class, &methods)? };
    *ready = true;
    Ok(())
}

/// Fails with `ERR_OCR_NOT_AVAILABLE` when the APK lacks ML Kit or the bridge class.
fn preflight(env: &mut Env<'_>) -> Result<(), OcrError> {
    for class in [TEXT_RECOGNITION, BRIDGE_CLASS] {
        if env.find_class(JNIString::new(class)).is_err() {
            clear(env);
            return Err(OcrError::new(ErrorKind::NotAvailable));
        }
    }
    Ok(())
}

/// Drops a pending exception so later JNI calls on the thread still work.
fn clear(env: &Env<'_>) {
    if env.exception_check() {
        env.exception_clear();
    }
}

/// Calls a no-argument method that returns an object.
fn object_of<'local>(
    env: &mut Env<'local>,
    node: &JObject<'_>,
    getter: &str,
    descriptor: &str,
) -> Result<JObject<'local>, OcrError> {
    let sig = method(descriptor)?;
    Ok(env
        .call_method(
            node,
            JNIString::new(getter),
            MethodSignature::from(&sig),
            &[],
        )?
        .l()?)
}

/// Calls a static method that returns an object.
fn static_object<'local>(
    env: &mut Env<'local>,
    class: &str,
    name: &str,
    descriptor: &str,
    args: &[JValue],
) -> Result<JObject<'local>, OcrError> {
    let sig = method(descriptor)?;
    Ok(env
        .call_static_method(
            JNIString::new(class),
            JNIString::new(name),
            MethodSignature::from(&sig),
            args,
        )?
        .l()?)
}

/// Calls a static method that returns an int.
fn static_int(
    env: &mut Env<'_>,
    class: &str,
    name: &str,
    descriptor: &str,
    args: &[JValue],
) -> Result<i32, OcrError> {
    let sig = method(descriptor)?;
    Ok(env
        .call_static_method(
            JNIString::new(class),
            JNIString::new(name),
            MethodSignature::from(&sig),
            args,
        )?
        .i()?)
}

/// Reads a static field that holds an object.
fn static_field_object<'local>(
    env: &mut Env<'local>,
    class: &str,
    name: &str,
    descriptor: &str,
) -> Result<JObject<'local>, OcrError> {
    let sig = field(descriptor)?;
    Ok(env
        .get_static_field(
            JNIString::new(class),
            JNIString::new(name),
            FieldSignature::from(&sig),
        )?
        .l()?)
}

/// Reads an int field.
fn int_field(env: &mut Env<'_>, node: &JObject<'_>, name: &str) -> Result<i32, OcrError> {
    let sig = field(INT_FIELD)?;
    Ok(env
        .get_field(node, JNIString::new(name), FieldSignature::from(&sig))?
        .i()?)
}

/// Reads a Java string; null reads as empty, the way ML Kit's nullable getters report nothing.
fn java_string(env: &Env<'_>, value: JObject<'_>) -> Result<String, OcrError> {
    if value.is_null() {
        return Ok(String::new());
    }
    let string = env.cast_local::<JString>(value)?;
    Ok(env.get_string(string)?.to_string())
}

/// A Java object's class in `Class.getName()` form.
fn class_name(env: &mut Env<'_>, object: &JObject<'_>) -> Result<String, OcrError> {
    let class = object_of(env, object, "getClass", CLASS_RETURN)?;
    let name = object_of(env, &class, "getName", STRING_RETURN)?;
    java_string(env, name)
}

/// The input as an Android bitmap; undecodable bytes are `ERR_OCR_INVALID_IMAGE`.
fn bitmap<'local>(env: &mut Env<'local>, image: &ImageInput) -> Result<JObject<'local>, OcrError> {
    match image {
        ImageInput::Encoded(bytes) => {
            let array = env.new_byte_array(bytes.len())?;
            // SAFETY: `jbyte` is i8 and has u8's layout; only the sign bit's meaning differs.
            let raw =
                unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<jbyte>(), bytes.len()) };
            env.set_byte_array_region(&array, 0, raw)?;
            let bitmap = static_object(
                env,
                BITMAP_FACTORY,
                "decodeByteArray",
                DECODE_BYTE_ARRAY,
                &[
                    JValue::Object(&array),
                    JValue::Int(0),
                    JValue::Int(bytes.len() as jint),
                ],
            )?;
            if bitmap.is_null() {
                return Err(OcrError::new(ErrorKind::InvalidImage));
            }
            Ok(bitmap)
        }
        ImageInput::Rgba {
            width,
            height,
            data,
        } => {
            let pixels = rgba_to_argb(data);
            let array = env.new_int_array(pixels.len())?;
            env.set_int_array_region(&array, 0, &pixels)?;
            let config = static_field_object(env, BITMAP_CONFIG, "ARGB_8888", CONFIG_FIELD)?;
            static_object(
                env,
                BITMAP,
                "createBitmap",
                CREATE_BITMAP,
                &[
                    JValue::Object(&array),
                    JValue::Int(*width as jint),
                    JValue::Int(*height as jint),
                    JValue::Object(&config),
                ],
            )
        }
    }
}

/// The bitmap as ML Kit's input image; rotation 0, because the crate's pixels carry no rotation.
fn input_image<'local>(
    env: &mut Env<'local>,
    bitmap: &JObject<'_>,
) -> Result<JObject<'local>, OcrError> {
    static_object(
        env,
        INPUT_IMAGE,
        "fromBitmap",
        FROM_BITMAP,
        &[JValue::Object(bitmap), JValue::Int(0)],
    )
}

/// A fresh ML Kit client for the script's options.
fn recognizer<'local>(env: &mut Env<'local>, script: Script) -> Result<JObject<'local>, OcrError> {
    let spec = options_for_script(script);
    let options = match spec.build {
        Build::Default => {
            let descriptor = format!("L{};", spec.class);
            static_field_object(env, spec.class, "DEFAULT_OPTIONS", &descriptor)?
        }
        Build::Builder => {
            let builder_class = format!("{}$Builder", spec.class);
            let ctor = method("()V")?;
            let builder = env.new_object(
                JNIString::new(&builder_class),
                MethodSignature::from(&ctor),
                &[],
            )?;
            let build = method(&format!("()L{};", spec.class))?;
            env.call_method(
                &builder,
                JNIString::new("build"),
                MethodSignature::from(&build),
                &[],
            )?
            .l()?
        }
    };
    let get_client = method(GET_CLIENT)?;
    Ok(env
        .call_static_method(
            JNIString::new(TEXT_RECOGNITION),
            JNIString::new("getClient"),
            MethodSignature::from(&get_client),
            &[JValue::Object(&options)],
        )?
        .l()?)
}

/// Closes an ML Kit client; errors are ignored, because the answer is already decided and
/// `close` is idempotent.
fn close(env: &mut Env<'_>, client: &JObject<'_>) {
    let Ok(sig) = method(CLOSE) else {
        return;
    };
    let _ = env.call_method(
        client,
        JNIString::new("close"),
        MethodSignature::from(&sig),
        &[],
    );
}

/// Starts the ML Kit task for the input.
fn process<'local>(
    env: &mut Env<'local>,
    client: &JObject<'local>,
    input: &JObject<'local>,
) -> Result<JObject<'local>, OcrError> {
    let sig = method(PROCESS)?;
    Ok(env
        .call_method(
            client,
            JNIString::new("process"),
            MethodSignature::from(&sig),
            &[JValue::Object(input)],
        )?
        .l()?)
}

/// Creates the bridge object that carries the call's id.
fn bridge_object(env: &mut Env<'_>, id: u64) -> Result<JObject<'_>, OcrError> {
    let sig = method(BRIDGE_CTOR)?;
    env.new_object(
        JNIString::new(BRIDGE_CLASS),
        MethodSignature::from(&sig),
        &[JValue::Long(id as jlong)],
    )
}

/// Arms one task's listeners; `Err` leaves nothing started and `done` uncalled, and the client
/// is closed on every failure path, because `getClient` has already created it.
fn arm(
    env: &mut Env<'_>,
    client: JObject<'_>,
    input: &JObject<'_>,
    done: RecognitionDone,
) -> Result<(), OcrError> {
    let client = env.new_global_ref(client)?;
    let task = match process(env, client.as_obj(), input) {
        Ok(task) => task,
        Err(error) => {
            close(env, client.as_obj());
            return Err(error);
        }
    };
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let callback = match bridge_object(env, id) {
        Ok(callback) => callback,
        Err(error) => {
            close(env, client.as_obj());
            return Err(error);
        }
    };
    // The entry goes in before the listeners: a task that is already complete fires a listener
    // inline, and the native callback must find the call to deliver its one answer.
    CALLS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(id, Pending { done, client });
    let armed = add_listener(
        env,
        &task,
        "addOnSuccessListener",
        ADD_ON_SUCCESS,
        &callback,
    )
    .and_then(|()| {
        add_listener(
            env,
            &task,
            "addOnFailureListener",
            ADD_ON_FAILURE,
            &callback,
        )
    });
    match armed {
        Ok(()) => Ok(()),
        Err(error) => {
            match CALLS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&id)
            {
                Some(pending) => {
                    close(env, pending.client.as_obj());
                    Err(error)
                }
                // The success listener already delivered the one answer inline.
                None => Ok(()),
            }
        }
    }
}

/// Adds one listener to a task.
fn add_listener(
    env: &mut Env<'_>,
    task: &JObject<'_>,
    name: &str,
    descriptor: &str,
    callback: &JObject<'_>,
) -> Result<(), OcrError> {
    let sig = method(descriptor)?;
    env.call_method(
        task,
        JNIString::new(name),
        MethodSignature::from(&sig),
        &[JValue::Object(callback)],
    )?;
    Ok(())
}

/// Starts one recognition on the calling thread, attached to the VM; `Err` means nothing was
/// started and `done` is never called.
fn start(
    env: &mut Env<'_>,
    image: &ImageInput,
    script: Script,
    done: RecognitionDone,
) -> Result<(), OcrError> {
    // Preflight first, so a missing class answers ERR_OCR_NOT_AVAILABLE, not a backend error.
    preflight(env)?;
    register_bridge(env)?;
    let bitmap = bitmap(env, image)?;
    let input = input_image(env, &bitmap)?;
    arm(env, recognizer(env, script)?, &input, done)
}

/// Runs `answer` and hands its result to the waiting call's `done`, closing the client. The
/// registry lookup is the exactly-once gate; a cancelled call is absent and stays dropped.
fn deliver(
    env: &mut Env<'_>,
    id: u64,
    answer: impl FnOnce(&mut Env<'_>) -> Result<RecognizedText, OcrError>,
) {
    let Some(pending) = CALLS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&id)
    else {
        return;
    };
    // A panic on the listener thread must not cross into the JVM or swallow the one answer.
    let answer = catch_unwind(AssertUnwindSafe(|| answer(env)))
        .unwrap_or_else(|_| Err(OcrError::new(ErrorKind::Backend)));
    close(env, pending.client.as_obj());
    (pending.done)(answer);
}

/// One coded error for a failed task; an ML Kit exception keeps its platform code, which
/// `getCode` only has on that one class.
fn task_failure(env: &mut Env<'_>, failure: &JObject<'_>) -> OcrError {
    let class = class_name(env, failure).unwrap_or_default();
    let mut code = None;
    if class == MLKIT_EXCEPTION
        && let Ok(sig) = method(INT_RETURN)
    {
        match env.call_method(
            failure,
            JNIString::new("getCode"),
            MethodSignature::from(&sig),
            &[],
        ) {
            Ok(value) => code = value.i().ok(),
            Err(_) => clear(env),
        }
    }
    map_task_failure(&class, code)
}

/// `NativeCallback.succeeded` - ML Kit's success listener, on the JVM's listener thread.
unsafe extern "system" fn succeeded(
    mut unowned: EnvUnowned<'_>,
    _this: JObject<'_>,
    pointer: jlong,
    text: JObject<'_>,
) {
    unowned
        .with_env(|env| -> jni::errors::Result<()> {
            deliver(env, pointer as u64, |env| read_text(env, &text));
            Ok(())
        })
        .into_outcome();
}

/// `NativeCallback.failed` - ML Kit's failure listener, on the JVM's listener thread.
unsafe extern "system" fn failed(
    mut unowned: EnvUnowned<'_>,
    _this: JObject<'_>,
    pointer: jlong,
    failure: JObject<'_>,
) {
    unowned
        .with_env(|env| -> jni::errors::Result<()> {
            deliver(env, pointer as u64, |env| Err(task_failure(env, &failure)));
            Ok(())
        })
        .into_outcome();
}

fn list_len(env: &mut Env<'_>, list: &JObject<'_>) -> Result<usize, OcrError> {
    let sig = method(INT_RETURN)?;
    let length = env.call_method(
        list,
        JNIString::new("size"),
        MethodSignature::from(&sig),
        &[],
    )?;
    Ok(length.i()?.max(0) as usize)
}

fn list_get<'local>(
    env: &mut Env<'local>,
    list: &JObject<'_>,
    index: usize,
) -> Result<JObject<'local>, OcrError> {
    let sig = method(LIST_GET)?;
    Ok(env
        .call_method(
            list,
            JNIString::new("get"),
            MethodSignature::from(&sig),
            &[JValue::Int(index as jint)],
        )?
        .l()?)
}

/// ML Kit's `Text` as the crate's result tree.
fn read_text(env: &mut Env<'_>, text: &JObject<'_>) -> Result<RecognizedText, OcrError> {
    Ok(RecognizedText {
        text: java_string(env, object_of(env, text, "getText", STRING_RETURN)?)?,
        blocks: read_blocks(env, &object_of(env, text, "getTextBlocks", GET_LIST)?)?,
    })
}

fn read_blocks(env: &mut Env<'_>, blocks: &JObject<'_>) -> Result<Vec<TextBlock>, OcrError> {
    let count = list_len(env, blocks)?;
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        let block = list_get(env, blocks, index)?;
        let lines = object_of(env, &block, "getLines", GET_LIST)?;
        out.push(TextBlock {
            text: java_string(env, object_of(env, &block, "getText", STRING_RETURN)?)?,
            lines: read_lines(env, &lines)?,
            bounding_box: rect_of(env, &block)?,
            languages: languages_of(env, &block)?,
            corner_points: points_of(env, &block)?,
        });
    }
    Ok(out)
}

fn read_lines(env: &mut Env<'_>, lines: &JObject<'_>) -> Result<Vec<TextLine>, OcrError> {
    let count = list_len(env, lines)?;
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        let line = list_get(env, lines, index)?;
        let elements = object_of(env, &line, "getElements", GET_LIST)?;
        out.push(TextLine {
            text: java_string(env, object_of(env, &line, "getText", STRING_RETURN)?)?,
            elements: read_elements(env, &elements)?,
            bounding_box: rect_of(env, &line)?,
            languages: languages_of(env, &line)?,
            corner_points: points_of(env, &line)?,
            confidence: boxed_float_of(env, &line, "getConfidence")?,
            angle: boxed_float_of(env, &line, "getAngle")?,
        });
    }
    Ok(out)
}

fn read_elements(env: &mut Env<'_>, elements: &JObject<'_>) -> Result<Vec<TextElement>, OcrError> {
    let count = list_len(env, elements)?;
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        let element = list_get(env, elements, index)?;
        let symbols = object_of(env, &element, "getSymbols", GET_LIST)?;
        out.push(TextElement {
            text: java_string(env, object_of(env, &element, "getText", STRING_RETURN)?)?,
            symbols: read_symbols(env, &symbols)?,
            bounding_box: rect_of(env, &element)?,
            languages: languages_of(env, &element)?,
            corner_points: points_of(env, &element)?,
            confidence: boxed_float_of(env, &element, "getConfidence")?,
            angle: boxed_float_of(env, &element, "getAngle")?,
        });
    }
    Ok(out)
}

fn read_symbols(env: &mut Env<'_>, symbols: &JObject<'_>) -> Result<Vec<TextSymbol>, OcrError> {
    let count = list_len(env, symbols)?;
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        let symbol = list_get(env, symbols, index)?;
        out.push(TextSymbol {
            text: java_string(env, object_of(env, &symbol, "getText", STRING_RETURN)?)?,
            bounding_box: rect_of(env, &symbol)?,
            languages: languages_of(env, &symbol)?,
            corner_points: points_of(env, &symbol)?,
            confidence: boxed_float_of(env, &symbol, "getConfidence")?,
            angle: boxed_float_of(env, &symbol, "getAngle")?,
        });
    }
    Ok(out)
}

/// A node's bounding box; ML Kit reports null for nodes it could not place.
fn rect_of(env: &mut Env<'_>, node: &JObject<'_>) -> Result<Rect, OcrError> {
    let rect = object_of(env, node, "getBoundingBox", GET_RECT)?;
    if rect.is_null() {
        return Ok(Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        });
    }
    Ok(Rect {
        left: int_field(env, &rect, "left")?,
        top: int_field(env, &rect, "top")?,
        right: int_field(env, &rect, "right")?,
        bottom: int_field(env, &rect, "bottom")?,
    })
}

/// A node's corner points, read through `java.lang.reflect.Array` because the JNI crate has no
/// plain path over an object array's elements.
fn points_of(env: &mut Env<'_>, node: &JObject<'_>) -> Result<Vec<Point>, OcrError> {
    let points = object_of(env, node, "getCornerPoints", GET_POINTS)?;
    if points.is_null() {
        return Ok(Vec::new());
    }
    let length = static_int(
        env,
        REFLECT_ARRAY,
        "getLength",
        ARRAY_LENGTH,
        &[JValue::Object(&points)],
    )?;
    let mut out = Vec::new();
    for index in 0..length {
        let point = static_object(
            env,
            REFLECT_ARRAY,
            "get",
            ARRAY_GET,
            &[JValue::Object(&points), JValue::Int(index)],
        )?;
        out.push(Point {
            x: int_field(env, &point, "x")?,
            y: int_field(env, &point, "y")?,
        });
    }
    Ok(out)
}

/// ML Kit reports one recognized language per node; the crate's tree keeps it in a list.
fn languages_of(env: &mut Env<'_>, node: &JObject<'_>) -> Result<Vec<String>, OcrError> {
    Ok(vec![java_string(
        env,
        object_of(env, node, "getRecognizedLanguage", STRING_RETURN)?,
    )?])
}

/// A boxed `java.lang.Float` getter; ML Kit gives confidence and angle only where it has them.
fn boxed_float_of(
    env: &mut Env<'_>,
    node: &JObject<'_>,
    getter: &str,
) -> Result<Option<f32>, OcrError> {
    let value = object_of(env, node, getter, GET_BOXED_FLOAT)?;
    if value.is_null() {
        return Ok(None);
    }
    let sig = method(FLOAT_VALUE)?;
    let float = env.call_method(
        &value,
        JNIString::new("floatValue"),
        MethodSignature::from(&sig),
        &[],
    )?;
    Ok(Some(float.f()?))
}

impl Backend for NativeBackend {
    fn supported_languages(&self) -> BackendFuture<'_, Result<Vec<String>, OcrError>> {
        Box::pin(async { Ok(supported()) })
    }

    fn start_recognition(
        &self,
        image: &ImageInput,
        options: &RecognitionOptions,
        done: RecognitionDone,
    ) -> Result<(), OcrError> {
        ensure_context()?;
        vm()?.attach_current_thread(|env| start(env, image, options.script, done))
    }

    fn cancel_recognition(&self) {
        let pending: Vec<Pending> = CALLS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain()
            .collect();
        if pending.is_empty() {
            return;
        }
        let Ok(vm) = vm() else {
            return;
        };
        // ML Kit cannot cancel a running task; closing each client stops what can be stopped,
        // and a listener that fires late finds no entry, so nothing resolves twice.
        let _ = vm.attach_current_thread(|env| {
            for call in pending {
                close(env, call.client.as_obj());
            }
            Ok::<(), OcrError>(())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_script_maps_to_its_ml_kit_options_class() {
        let cases = [
            (
                Script::Latin,
                "com/google/mlkit/vision/text/latin/TextRecognizerOptions",
                Build::Default,
            ),
            (
                Script::Chinese,
                "com/google/mlkit/vision/text/chinese/ChineseTextRecognizerOptions",
                Build::Builder,
            ),
            (
                Script::Devanagiri,
                "com/google/mlkit/vision/text/devanagari/DevanagariTextRecognizerOptions",
                Build::Builder,
            ),
            (
                Script::Japanese,
                "com/google/mlkit/vision/text/japanese/JapaneseTextRecognizerOptions",
                Build::Builder,
            ),
            (
                Script::Korean,
                "com/google/mlkit/vision/text/korean/KoreanTextRecognizerOptions",
                Build::Builder,
            ),
        ];
        for (script, class, build) in cases {
            let spec = options_for_script(script);
            assert_eq!(spec.class, class);
            assert_eq!(spec.build, build);
        }
    }

    #[test]
    fn rgba_bytes_pack_into_argb_ints() {
        let pixels = rgba_to_argb(&[0x11, 0x22, 0x33, 0xFF, 0x00, 0x10, 0x20, 0x80]);
        assert_eq!(
            pixels,
            vec![0xFF_11_22_33u32 as jint, 0x80_00_10_20u32 as jint]
        );
        assert_eq!(rgba_to_argb(&[0x01, 0x02, 0x03, 0x04, 0x05]).len(), 1);
    }

    #[test]
    fn every_signature_string_parses() {
        let methods = [
            GET_CLIENT,
            PROCESS,
            ADD_ON_SUCCESS,
            ADD_ON_FAILURE,
            DECODE_BYTE_ARRAY,
            CREATE_BITMAP,
            FROM_BITMAP,
            STRING_RETURN,
            CLASS_RETURN,
            GET_LIST,
            GET_RECT,
            GET_POINTS,
            GET_BOXED_FLOAT,
            FLOAT_VALUE,
            INT_RETURN,
            CLOSE,
            BRIDGE_CTOR,
            NATIVE_SUCCESS,
            NATIVE_FAILURE,
            LIST_GET,
            ARRAY_LENGTH,
            ARRAY_GET,
            CURRENT_APPLICATION,
        ];
        for descriptor in methods {
            assert!(
                RuntimeMethodSignature::from_str(descriptor).is_ok(),
                "{descriptor}"
            );
        }
        for descriptor in [CONFIG_FIELD, INT_FIELD] {
            assert!(
                RuntimeFieldSignature::from_str(descriptor).is_ok(),
                "{descriptor}"
            );
        }
    }

    #[test]
    fn the_load_bearing_signatures_say_exactly_this() {
        assert_eq!(
            GET_CLIENT,
            "(Lcom/google/mlkit/vision/text/TextRecognizerOptions;)Lcom/google/mlkit/vision/text/TextRecognizer;"
        );
        assert_eq!(
            PROCESS,
            "(Lcom/google/mlkit/vision/common/InputImage;)Lcom/google/android/gms/tasks/Task;"
        );
        assert_eq!(DECODE_BYTE_ARRAY, "([BII)Landroid/graphics/Bitmap;");
        assert_eq!(
            CREATE_BITMAP,
            "([IIILandroid/graphics/Bitmap$Config;)Landroid/graphics/Bitmap;"
        );
        assert_eq!(
            FROM_BITMAP,
            "(Landroid/graphics/Bitmap;I)Lcom/google/mlkit/vision/common/InputImage;"
        );
        assert_eq!(NATIVE_SUCCESS, "(JLjava/lang/Object;)V");
        assert_eq!(NATIVE_FAILURE, "(JLjava/lang/Exception;)V");
    }

    #[test]
    fn supported_languages_are_the_five_script_languages() {
        assert_eq!(supported(), ["en-US", "zh-Hans", "hi-IN", "ja-JP", "ko-KR"]);
    }

    #[test]
    fn an_ml_kit_failure_keeps_its_platform_code() {
        let error = map_task_failure(MLKIT_EXCEPTION, Some(13));
        assert_eq!(error.kind(), ErrorKind::Backend);
        assert_eq!(error.status(), Some(13));
        assert_eq!(map_task_failure("java.io.IOException", None).status(), None);
        assert_eq!(map_task_failure(MLKIT_EXCEPTION, None).status(), None);
        assert_eq!(
            map_task_failure("java.io.IOException", Some(7)).kind(),
            ErrorKind::Backend
        );
    }
}
