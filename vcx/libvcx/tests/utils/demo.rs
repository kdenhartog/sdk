extern crate vcx;
extern crate tempfile;
extern crate libc;
extern crate serde_json;

use utils::timeout::TimeoutUtils;
use utils::cstring::CStringUtils;
use std::ptr;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use self::libc::c_char;
use std::thread;
use std::time::Duration;
use std::ffi::CString;
use vcx::api;
use std::sync::Mutex;
use std::sync::mpsc::channel;

lazy_static! {
    static ref COMMAND_HANDLE_COUNTER: AtomicUsize = ATOMIC_USIZE_INIT;
}

#[allow(unused_assignments)]
#[allow(unused_variables)]
#[allow(dead_code)]
pub extern "C" fn generic_cb(command_handle:u32, err:u32) {
    if err != 0 {panic!("failed connect: {}", err)}
    println!("connection established!");
}


#[allow(dead_code)]
pub fn create_claim_offer(claim_name: &str, source_id: &str, claim_data_value: serde_json::Value, issuer_did: &str, schema_seq_no: u32) -> (u32, u32){
    let source_id_cstring = CString::new(source_id).unwrap();
    let (sender, receiver) = channel();
    let cb = Box::new(move|err, claim_handle|{sender.send((err, claim_handle)).unwrap();});
    let (command_handle, cb) = closure_to_create_claim(cb);
    let claim_data_str = serde_json::to_string(&claim_data_value).unwrap();
    let claim_data_cstring = CString::new(claim_data_str).unwrap();
    #[allow(unused_variables)]
    let issuer_did_cstring = CString::new(issuer_did).unwrap();
    let claim_name_cstring = CString::new(claim_name).unwrap();
    let rc = api::issuer_claim::vcx_issuer_create_claim(command_handle,
                                                        source_id_cstring.as_ptr(),
                                                        schema_seq_no,
                                                        ptr::null(),
                                                        claim_data_cstring.as_ptr(),
                                                        claim_name_cstring.as_ptr(),
                                                        cb);
    assert_eq!(rc, 0);
    receiver.recv_timeout(TimeoutUtils::long_timeout()).unwrap()
}

#[allow(dead_code)]
pub fn send_claim_offer(claim_handle: u32, connection_handle: u32) -> u32 {
    let (sender, receiver) = channel();
    let cb = Box::new(move|err|{sender.send(err).unwrap();});
    let (command_handle, cb) = closure_to_send_claim_object(cb);
    let rc = api::issuer_claim::vcx_issuer_send_claim_offer(command_handle,
                                                            claim_handle,
                                                            connection_handle,
                                                            cb);
    assert_eq!(rc,0);
    receiver.recv_timeout(TimeoutUtils::long_timeout()).unwrap()
}

#[allow(dead_code)]
pub fn send_claim(claim_handle: u32, connection_handle: u32) -> u32 {
    let (sender, receiver) = channel();
    let cb = Box::new(move|err|{sender.send(err).unwrap();});
    let (command_handle, cb) = closure_to_send_claim_object(cb);
    let rc = api::issuer_claim::vcx_issuer_send_claim(command_handle, claim_handle, connection_handle, cb);
    assert_eq!(rc,0);
    receiver.recv_timeout(TimeoutUtils::long_timeout()).unwrap()

}
#[allow(dead_code)]
pub fn deserialize_vcx_object(serialized_connection: &str,f:extern fn(u32, *const c_char, Option<extern fn(u32, u32, u32)>) ->u32 ) -> u32{
    fn closure_to_deserialize_connection(closure: Box<FnMut(u32, u32) + Send>) ->
    (u32,  Option<extern fn( command_handle: u32,
                             err: u32 ,
                             connection_handle: u32)>) {
        lazy_static! { static ref CALLBACK_DESERIALIE_CONNECTION: Mutex<HashMap<u32,
                                        Box<FnMut(u32, u32) + Send>>> = Default::default(); }

        extern "C" fn callback(command_handle: u32, err: u32, connection_handle: u32) {
            let mut callbacks = CALLBACK_DESERIALIE_CONNECTION.lock().unwrap();
            let mut cb = callbacks.remove(&command_handle).unwrap();
            cb(err, connection_handle)
        }

        let mut callbacks = CALLBACK_DESERIALIE_CONNECTION.lock().unwrap();
        let command_handle = (COMMAND_HANDLE_COUNTER.fetch_add(1, Ordering::SeqCst) + 1) as u32;
        callbacks.insert(command_handle, closure);

        (command_handle, Some(callback))
    }
    let (sender, receiver) = channel();
    let cb = Box::new(move|err, handle|{sender.send((err,handle)).unwrap();});
    let (command_handle, cb) = closure_to_deserialize_connection(cb);
    let rc = f(command_handle,
               CStringUtils::string_to_cstring(String::from(serialized_connection)).as_ptr(),
               cb);
    assert_eq!(rc,0);
    let (err, connection_handle) = receiver.recv_timeout(TimeoutUtils::long_timeout()).unwrap();
    assert_eq!(err,0);
    connection_handle

}

#[allow(dead_code)]
pub fn serialize_vcx_object(connection_handle: u32, f:extern fn(u32, u32, Option<extern fn(u32, u32, *const c_char)> ) ->u32) -> u32{
    fn closure_to_serialize_connection(closure: Box<FnMut(u32) + Send>) ->
    (u32, Option<extern fn( command_handle: u32, err: u32 , claim_string: *const c_char)>) {
        lazy_static! { static ref CALLBACKS_SERIALIZE_CONNECTION: Mutex<HashMap<u32,
                                        Box<FnMut(u32) + Send>>> = Default::default(); }

        extern "C" fn callback(command_handle: u32, err: u32, claim_string: *const c_char) {
            let mut callbacks = CALLBACKS_SERIALIZE_CONNECTION.lock().unwrap();
            let mut cb = callbacks.remove(&command_handle).unwrap();
            assert_eq!(err, 0);
            if claim_string.is_null() {
                panic!("claim_string is empty");
            }
            check_useful_c_str!(claim_string, ());
            println!("successfully called serialize_cb: {}", claim_string);
            cb(err)
        }

        let mut callbacks = CALLBACKS_SERIALIZE_CONNECTION.lock().unwrap();
        let command_handle = (COMMAND_HANDLE_COUNTER.fetch_add(1, Ordering::SeqCst) + 1) as u32;
        callbacks.insert(command_handle, closure);

        (command_handle, Some(callback))
    }
    let (sender, receiver) = channel();
    let cb = Box::new(move |err|{sender.send(err).unwrap();});
    let (command_handle, cb) = closure_to_serialize_connection(cb);
    let rc = f(command_handle,
               connection_handle,
               cb);

    assert_eq!(rc, 0);
    receiver.recv_timeout(TimeoutUtils::long_timeout()).unwrap()
}


#[allow(dead_code)]
pub fn invite_details_vcx_object(connection_handle: u32, f:extern fn(u32, u32, bool, Option<extern fn(u32, u32, *const c_char)>) -> u32) -> u32 {
    fn closure_to_vcx_connection(closure: Box<FnMut(u32) + Send>) ->
    (u32, Option<extern fn( command_handle: u32, err: u32 , details: *const c_char)>) {
        lazy_static! { static ref CALLBACKS_SERIALIZE_CONNECTION: Mutex<HashMap<u32,
                                        Box<FnMut(u32) + Send>>> = Default::default(); }

        extern "C" fn callback(command_handle: u32, err: u32, details: *const c_char) {
            let mut callbacks = CALLBACKS_SERIALIZE_CONNECTION.lock().unwrap();
            let mut cb = callbacks.remove(&command_handle).unwrap();
            assert_eq!(err, 0);
            if details.is_null() {
                panic!("details is empty");
            }
            check_useful_c_str!(details, ());
            println!("\n*************\nQR CODE JSON: \n{}\n*************", details);
            cb(err)
        }

        let mut callbacks = CALLBACKS_SERIALIZE_CONNECTION.lock().unwrap();
        let command_handle = (COMMAND_HANDLE_COUNTER.fetch_add(1, Ordering::SeqCst) + 1) as u32;
        callbacks.insert(command_handle, closure);

        (command_handle, Some(callback))
    }
    let (sender, receiver) = channel();
    let cb = Box::new(move |err|{sender.send(err).unwrap();});
    let (command_handle, cb) = closure_to_vcx_connection(cb);
    let rc = f(command_handle,
               connection_handle,
               true,
               cb);

    assert_eq!(rc, 0);
    receiver.recv_timeout(TimeoutUtils::long_timeout()).unwrap()
}

#[allow(dead_code)]
pub fn wait_for_updated_state(handle: u32, target_state:u32, f: extern fn(u32, u32, Option<extern fn(u32, u32, u32)>)->u32)->u32{
    //  Update State, wait for connection *********************************************
    let mut state = 0;
    while state != target_state {
        let (sender, receiver) = channel();
        let (command_handle, cb) = closure_to_update_state(Box::new(move |state| { sender.send(state).unwrap(); }));
        thread::sleep(Duration::from_millis(5000));
        let err = f(command_handle, handle, cb);
        assert_eq!(err,0);
        state = receiver.recv_timeout(TimeoutUtils::long_timeout()).unwrap();
    }
    state
}

#[allow(dead_code)]
pub fn closure_to_create_connection_cb(closure: Box<FnMut(u32, u32) + Send>) ->
(u32,
 Option<extern fn(
     command_handle: u32,
     err: u32,
     connection_handle: u32)>) {
    lazy_static! {
            static ref CALLBACKS_CREATE_CONNECTION: Mutex<HashMap<u32, Box<FnMut(u32, u32) + Send>>> = Default::default();
        }

    extern "C" fn callback(command_handle: u32, err: u32, connection_handle: u32) {
        let mut callbacks = CALLBACKS_CREATE_CONNECTION.lock().unwrap();
        let mut cb = callbacks.remove(&command_handle).unwrap();
        cb(err, connection_handle)
    }

    let mut callbacks = CALLBACKS_CREATE_CONNECTION.lock().unwrap();
    let command_handle = (COMMAND_HANDLE_COUNTER.fetch_add(1, Ordering::SeqCst) + 1) as u32;
    callbacks.insert(command_handle, closure);

    (command_handle, Some(callback))
}

#[allow(dead_code)]
pub fn closure_to_connect_cb(closure: Box<FnMut(u32) + Send>) -> (u32,
                                                                  Option<extern fn(
                                                                      command_handle: u32,
                                                                      err: u32,
                                                                      details: *const c_char)>) {
    lazy_static! {
        static ref CALLBACKS: Mutex<HashMap<u32, Box<FnMut(u32) + Send>>> = Default::default();
    }
    // this is the only difference between the two closure converters
    #[allow(unused_variables)]
    extern "C" fn callback(command_handle: u32, err: u32, details: *const c_char) {
        let mut callbacks = CALLBACKS.lock().unwrap();
        let mut cb = callbacks.remove(&command_handle).unwrap();
        cb(err)
    }

    let mut callbacks = CALLBACKS.lock().unwrap();
    let command_handle = (COMMAND_HANDLE_COUNTER.fetch_add(1, Ordering::SeqCst) + 1) as u32;
    callbacks.insert(command_handle, closure);

    (command_handle, Some(callback))
}

#[allow(dead_code)]
pub fn closure_to_update_state(closure: Box<FnMut(u32) + Send>) ->
(u32,
 Option<extern fn(
     command_handle: u32,
     err: u32,
     connection_handle: u32)>) {
    lazy_static! { static ref CALLBACKS_GET_STATE: Mutex<HashMap<u32, Box<FnMut(u32) + Send>>> = Default::default(); }

    #[allow(unused_variables)]
    extern "C" fn callback(command_handle: u32, err: u32, state: u32) {
        let mut callbacks = CALLBACKS_GET_STATE.lock().unwrap();
        let mut cb = callbacks.remove(&command_handle).unwrap();
        cb(state)
    }

    let mut callbacks = CALLBACKS_GET_STATE.lock().unwrap();
    let command_handle = (COMMAND_HANDLE_COUNTER.fetch_add(1, Ordering::SeqCst) + 1) as u32;
    callbacks.insert(command_handle, closure);

    (command_handle, Some(callback))
}

#[allow(dead_code)]
pub fn closure_to_create_claim(closure: Box<FnMut(u32, u32) + Send>) ->
(u32, Option<extern fn( command_handle: u32, err: u32, claim_handle: u32)>) {
    lazy_static! { static ref CALLBACKS_CREATE_CLAIM: Mutex<HashMap<u32, Box<FnMut(u32, u32) + Send>>> = Default::default(); }

    extern "C" fn callback(command_handle: u32, err: u32, claim_handle: u32) {
        let mut callbacks = CALLBACKS_CREATE_CLAIM.lock().unwrap();
        let mut cb = callbacks.remove(&command_handle).unwrap();
        cb(err, claim_handle)
    }

    let mut callbacks = CALLBACKS_CREATE_CLAIM.lock().unwrap();
    let command_handle = (COMMAND_HANDLE_COUNTER.fetch_add(1, Ordering::SeqCst) + 1) as u32;
    callbacks.insert(command_handle, closure);

    (command_handle, Some(callback))
}

#[allow(dead_code)]
pub fn closure_to_create_claimdef(closure: Box<FnMut(u32, u32) + Send>) ->
(u32, Option<extern fn( command_handle: u32, err: u32, claimdef_handle: u32)>) {
    lazy_static! { static ref CALLBACKS_CREATE_CLAIMDEF: Mutex<HashMap<u32, Box<FnMut(u32, u32) + Send>>> = Default::default(); }

    extern "C" fn callback(command_handle: u32, err: u32, claimdef_handle: u32) {
        let mut callbacks = CALLBACKS_CREATE_CLAIMDEF.lock().unwrap();
        let mut cb = callbacks.remove(&command_handle).unwrap();
        cb(err, claimdef_handle)
    }

    let mut callbacks = CALLBACKS_CREATE_CLAIMDEF.lock().unwrap();
    let command_handle = (COMMAND_HANDLE_COUNTER.fetch_add(1, Ordering::SeqCst) + 1) as u32;
    callbacks.insert(command_handle, closure);

    (command_handle, Some(callback))
}

#[allow(dead_code)]
pub fn closure_to_send_claim_object(closure: Box<FnMut(u32) + Send>) -> (u32, Option<extern fn(command_handle: u32, err: u32 )>) {
    lazy_static! { static ref CALLBACKS_SEND_CLAIM: Mutex<HashMap<u32, Box<FnMut(u32) + Send>>> = Default::default(); }

    extern "C" fn callback(command_handle: u32, err: u32) {
        let mut callbacks = CALLBACKS_SEND_CLAIM.lock().unwrap();
        let mut cb = callbacks.remove(&command_handle).unwrap();
        cb(err)
    }

    let mut callbacks = CALLBACKS_SEND_CLAIM.lock().unwrap();
    let command_handle = (COMMAND_HANDLE_COUNTER.fetch_add(1, Ordering::SeqCst) + 1) as u32;
    callbacks.insert(command_handle, closure);

    (command_handle, Some(callback))
}

#[allow(dead_code)]
pub fn send_proof_request(proof_handle: u32, connection_handle: u32) -> u32 {
    let (sender, receiver) = channel();
    let cb = Box::new(move|err|{sender.send(err).unwrap();});
    let (command_handle, cb) = closure_to_send_claim_object(cb);
    let rc = api::proof::vcx_proof_send_request(command_handle, proof_handle, connection_handle, cb);
    assert_eq!(rc,0);
    receiver.recv_timeout(TimeoutUtils::long_timeout()).unwrap()

}
#[allow(dead_code)]
pub fn create_proof_request(source_id: &str, requested_attrs: &str) -> (u32, u32){
    let requested_attrs = CString::new(requested_attrs).unwrap();
    let source_id_cstring = CString::new(source_id).unwrap();
    let (sender, receiver) = channel();
    let cb = Box::new(move|err, claim_handle|{sender.send((err, claim_handle)).unwrap();});
    let (command_handle, cb) = closure_to_create_claim(cb);
    let predicates_cstring = CString::new("[]").unwrap();
    let proof_name_cstring = CString::new("proof name").unwrap();
    let rc = api::proof::vcx_proof_create(command_handle,
                                          source_id_cstring.as_ptr(),
                                          requested_attrs.as_ptr(),
                                          predicates_cstring.as_ptr(),
                                          proof_name_cstring.as_ptr(),
                                          cb);
    assert_eq!(rc, 0);
    receiver.recv_timeout(TimeoutUtils::long_timeout()).unwrap()
}


#[allow(dead_code)]
pub fn get_proof(proof_handle: u32, connection_handle: u32) -> u32 {
    fn closure_to_get_proof(closure: Box<FnMut(u32) + Send>) ->
    (u32, Option<extern fn( command_handle: u32, err: u32, proof_state: u32, proof_string: *const c_char)>) {
        lazy_static! { static ref CALLBACK_GET_PROOF: Mutex<HashMap<u32,
                                        Box<FnMut(u32) + Send>>> = Default::default(); }

        extern "C" fn callback(command_handle: u32, err: u32, proof_state: u32, proof_str: *const c_char) {
            let mut callbacks = CALLBACK_GET_PROOF.lock().unwrap();
            let mut cb = callbacks.remove(&command_handle).unwrap();

            assert_eq!(proof_state, 1);
            assert_eq!(err, 0);
            if proof_str.is_null() {
                panic!("proof_str is empty");
            }
            check_useful_c_str!(proof_str, ());
            println!("successfully called get_proof_cb: {}", proof_str);
            cb(err)
        }

        let mut callbacks = CALLBACK_GET_PROOF.lock().unwrap();
        let command_handle = (COMMAND_HANDLE_COUNTER.fetch_add(1, Ordering::SeqCst) + 1) as u32;
        callbacks.insert(command_handle, closure);

        (command_handle, Some(callback))
    }
    let (sender, receiver) = channel();
    let cb = Box::new(move |err|{sender.send(err).unwrap();});
    let (command_handle, cb) = closure_to_get_proof(cb);
    let rc = api::proof::vcx_get_proof(command_handle,
                                       proof_handle,
                                       connection_handle,
                                       cb);

    assert_eq!(rc, 0);
    receiver.recv_timeout(TimeoutUtils::long_timeout()).unwrap()

}

#[allow(dead_code)]
pub fn create_claimdef(source_id: &str, claimdef_name: &str, schema_seq_no: u32) -> (u32, u32){
    let source_id_cstring = CString::new(source_id).unwrap();
    let (sender, receiver) = channel();
    let cb = Box::new(move|err, claimdef_handle|{sender.send((err, claimdef_handle)).unwrap();});
    let (command_handle, cb) = closure_to_create_claimdef(cb);
    let claimdef_name_cstring = CString::new(claimdef_name).unwrap();
    let rc = api::claim_def::vcx_claimdef_create(command_handle,
                                                     source_id_cstring.as_ptr(),
                                                     claimdef_name_cstring.as_ptr(),
                                                        schema_seq_no,
                                                        ptr::null(),
                                                 false,
                                                     cb);
    assert_eq!(rc, 0);
    receiver.recv_timeout(TimeoutUtils::long_timeout()).unwrap()
}

#[allow(dead_code)]
pub fn create_schema(source_id: &str, schema_name: &str, schema_data: &str) -> (u32, u32, u32){
    let source_id_cstring = CString::new(source_id).unwrap();
    let (sender, receiver) = channel();
    let cb = Box::new(move|err, claimdef_handle|{sender.send((err, claimdef_handle)).unwrap();});
    let (command_handle, cb) = closure_to_create_claimdef(cb);
    let schema_name_cstring = CString::new(schema_name).unwrap();
    let schema_data_cstring = CString::new(schema_data).unwrap();
    let rc = api::schema::vcx_schema_create(command_handle,
                                                     source_id_cstring.as_ptr(),
                                                     schema_name_cstring.as_ptr(),
                                                     schema_data_cstring.as_ptr(),
                                                     cb);
    assert_eq!(rc, 0);
    let (rc, handle) = receiver.recv_timeout(TimeoutUtils::long_timeout()).unwrap();
    let schema_no = ::vcx::schema::get_sequence_num(handle).unwrap();
    (rc, handle, schema_no)
}