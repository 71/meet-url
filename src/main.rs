use std::collections::{hash_map::Entry, HashMap};

use rocket::{
    fairing::AdHoc,
    http::{
        hyper::header::{ACCESS_CONTROL_ALLOW_ORIGIN, HOST},
        Header, Status,
    },
    request::{FromRequest, Outcome},
    response::Redirect,
    tokio::{sync::Mutex, time::Instant},
    Request, State,
};

struct Code {
    value: String,
    updated: Instant,
}

impl Code {
    const EXPIRES_AFTER_N_SECS: u64 = 1_200;
}

type Rooms = HashMap<String, Code>;

struct Host(String);

#[rocket::async_trait]
impl<'a, 'r> FromRequest<'a, 'r> for Host {
    type Error = &'static str;

    async fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        match request.headers().get_one(HOST.as_str()) {
            Some(header) => Outcome::Success(Host(header.to_string())),
            None => Outcome::Failure((Status::BadRequest, "missing host header in request")),
        }
    }
}

#[rocket::get("/")]
fn redirect_to_github() -> Redirect {
    Redirect::permanent("https://github.com/71/meet-url")
}

#[rocket::get("/<room>/script")]
fn get_script(room: String, host: Host) -> String {
    format!("
javascript:(async function(room, host) {{
    if (typeof room !== 'string' || room.length === 0) {{
        return alert('invalid room name');
    }}
    if (location.origin !== 'https://meet.google.com') {{
        return alert('script must be run on https://meet.google.com');
    }}

    let resp = await fetch(`${{host}}/${{room}}/code`);

    if (resp.ok) {{
        const code = await resp.text(),
              suffix = location.search;

        return location.href = `https://meet.google.com/${{code}}${{suffix}}`;
    }}

    if (resp.status !== 404) {{
        return alert(`error ${{resp.status}}: ${{resp.statusText || 'unknown'}}`);
    }}

    const createMeetingButton = document.querySelector('li[aria-label=\"Create a meeting for later\"]')
                             ?? document.querySelector('li.VfPpkd-rymPhb-ibnC6b');

    createMeetingButton.click();

    let meetingCode;

    for (let i = 0; i < 20; i++) {{
        await new Promise((resolve) => setTimeout(resolve, 100));

        const meetingCodeBox = document.querySelector('div.Hayy8b');

        if (meetingCodeBox !== null) {{
            meetingCode = /[a-z]{{3}}-[a-z]{{4}}-[a-z]{{3}}/.exec(meetingCodeBox.textContent)[0];
            break;
        }}
    }}

    if (meetingCode === undefined) {{
        return alert('could not find meeting code in page');
    }}

    resp = await fetch(`${{host}}/${{room}}/code/${{meetingCode}}`, {{ method: 'POST' }});

    if (!resp.ok) {{
        return alert(`error ${{resp.status}}: ${{resp.statusText || 'unknown'}}`);
    }}

    const code = await resp.text(),
          suffix = location.search;

    return location.href = `https://meet.google.com/${{code}}${{suffix}}`;
}})('{}', 'https://{}')
", room, host.0).replace("  ", "").replace('\n', "")
}

#[rocket::get("/<room>")]
async fn get_room<'r>(room: String, state: State<'r, Mutex<Rooms>>) -> Redirect {
    let mut state = state.lock().await;

    if let Entry::Occupied(entry) = state.entry(room) {
        if entry.get().updated.elapsed().as_secs() <= Code::EXPIRES_AFTER_N_SECS {
            return Redirect::found(format!("https://meet.google.com/{}", entry.get().value));
        }

        entry.remove();
    }

    Redirect::to("https://meet.google.com/landing")
}

#[rocket::get("/<room>/code")]
async fn get_code<'r>(room: String, state: State<'r, Mutex<Rooms>>) -> Option<String> {
    let mut state = state.lock().await;

    if let Entry::Occupied(entry) = state.entry(room) {
        if entry.get().updated.elapsed().as_secs() <= Code::EXPIRES_AFTER_N_SECS {
            return Some(entry.get().value.clone());
        }

        entry.remove();
    }

    None
}

#[rocket::post("/<room>/code/<code>")]
async fn post_code<'r>(
    room: String,
    code: String,
    state: State<'r, Mutex<Rooms>>,
) -> Result<String, &'static str> {
    if code.as_bytes().len() != 12 {
        return Err("invalid code");
    }

    for &i in &[0, 1, 2, 4, 5, 6, 7, 9, 10, 11] {
        if !code.as_bytes()[i].is_ascii_lowercase() {
            return Err("invalid code");
        }
    }

    if code.as_bytes()[3] != b'-' || code.as_bytes()[8] != b'-' {
        return Err("invalid code");
    }

    let mut state = state.lock().await;
    state.insert(
        room,
        Code {
            value: code.clone(),
            updated: Instant::now(),
        },
    );

    Ok(code)
}

#[rocket::catch(404)]
fn not_found() -> &'static str {
    "not found"
}

#[rocket::launch]
fn rocket() -> rocket::Rocket {
    rocket::ignite()
        .attach(AdHoc::on_response("Post-process", |req, resp| {
            Box::pin(async move {
                if resp.status() == Status::Ok || resp.status() == Status::NotFound {
                    resp.set_header(Header::new(
                        ACCESS_CONTROL_ALLOW_ORIGIN.as_str(),
                        "https://meet.google.com",
                    ));
                }

                if let Some(user_id) = req.get_query_value::<u8>("u").and_then(Result::ok) {
                    if let Some(redirect_url) = resp.headers().get_one("Location") {
                        if redirect_url.starts_with("https://meet.google.com") {
                            let redirect_url = format!("{}?authuser={}", redirect_url, user_id);

                            resp.set_raw_header("Location", redirect_url);
                        }
                    }
                }
            })
        }))
        .manage(Mutex::new(Rooms::new()))
        .mount(
            "/",
            rocket::routes![
                redirect_to_github,
                get_script,
                get_room,
                get_code,
                post_code
            ],
        )
        .register(rocket::catchers![not_found])
}
