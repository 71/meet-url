# meet-url

`meet-url` is a [Rocket](https://rocket.rs/)-powered Rust server
that serves a **very** simple API for giving names to Google Meet
rooms.

It's ridiculously simple.

Launch a server by running `cargo run`, and:
- `/` will redirect to https://github.com/71/meet-url
- `/<name>` will redirect to the call with the given name if it
  exists, or to Google Meet otherwise.
- `/<name>/code` will return the actual code for the call with
  the given name, or `404` if no such name exists.
- `/<name>/code/<code>` (POST) will update the code for the call
  with the given name. The code must be like `abc-defg-hij`.
- `/<name>/script` returns a string that can be used as a bookmarklet
  on Google Meet. Press the bookmarklet to either open the room if it
  already exists, or create a new one and open it otherwise.

Room names expire after 20 minutes, but you may POST `/<name>/code/<code>`
at any time to refresh / keep alive the code.

## Usage examples

### For a host

The host would create a new room by going on Google Meet and executing
the bookmarklet at `/<name>/script`, or the following script (with the
placeholders replaced).

```js
(async function(room, host) {
    if (typeof room !== 'string' || room.length === 0) {
        return alert('invalid room name');
    }
    if (location.origin !== 'https://meet.google.com') {
        return alert('script must be run on https://meet.google.com');
    }

    let resp = await fetch(`${host}/${room}/code`);

    if (resp.ok) {
        const code = await resp.text(),
              suffix = location.search;

        return location.href = `https://meet.google.com/${code}${suffix}`;
    }

    if (resp.status !== 404) {
        return alert(`error ${resp.status}: ${resp.statusText || 'unknown'}`);
    }

    const createMeetingButton = document.querySelector('li[aria-label=\"Create a meeting for later\"]')
                             ?? document.querySelector('li.VfPpkd-rymPhb-ibnC6b');

    createMeetingButton.click();

    let meetingCode;

    for (let i = 0; i < 20; i++) {
        await new Promise((resolve) => setTimeout(resolve, 100));

        const meetingCodeBox = document.querySelector('div.Hayy8b');

        if (meetingCodeBox !== null) {
            meetingCode = /[a-z]{{3}}-[a-z]{{4}}-[a-z]{{3}}/.exec(meetingCodeBox.textContent)[0];
            break;
        }
    }

    if (meetingCode === undefined) {
        return alert('could not find meeting code in page');
    }

    resp = await fetch(`${host}/${room}/code/${meetingCode}`, { method: 'POST' });

    if (!resp.ok) {
        return alert(`error ${resp.status}: ${resp.statusText || 'unknown'}`);
    }

    const code = await resp.text(),
          suffix = location.search;

    return location.href = `https://meet.google.com/${code}${suffix}`;
})('<room name>', '<your server>')
```

Alternatively, the host can create a new call by navigating to
https://meet.google.com/new, and then execute the given script to upload
the code of the room:

```js
fetch("<your server>/<room name>/code" + location.pathname, { method: "POST" })
```

Once that's done, anyone can navigate to `<your server>/<room name>` to be
redirected to the meeting.

### For groups

For groups, users may join in different orders (rather than always having a
host join first). This is where the script / bookmarklets are useful. From
https://meet.google.com/landing, simply execute the script and your browser
will automatically navigate to the call, creating it and uploading its code
first if it doesn't already exist.

## Why?

I've been making many calls through Google Meet with friends, and other
services just haven't been as good (looking at you, Google Duo) as Google
Meet. Thing is, it gets a little tedious to create meetings and share codes
all the time, and I thought it would be nice to have our own link to start
meetings together.

## Security

There's no security / authentication, really. On my end I'm just running this
on a VPS and with HTTPS, so if you want "secure" rooms you can always just
give them really long names and hope nobody finds it. It doesn't matter that
much anyway, since attendees need to be approved in the meeting anyway.

## Can't you do better, though?

There are probably ways to make this even better, like removing the
requirement that the script must run on Google Meet or scheduling meetings
in Calendar or whatever, but heh. Good enough for me.
