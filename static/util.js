/*
util.js

Utility functions for Admins and Teachers (those doing heavy API call work).

  * The machinery for tracking which API requests have come back and displaying
    progress accordingly.
  * Some other stuff.

This script should be loaded synchronously at the bottom of the <BODY>, and
other scripts should be loaded with the DEFER attribute, to assure this loads
ahead of them.
*/
"use strict";

const UTIL = {
    ensure_on_load: function(callback) {
        if(document.readystate == "complete") {
            callback();
        } else {
            window.addEventListener("load", callback);
        }
    },

    clear: function(elt) {
        while(elt.firstChild) {
            recursive_clear(elt.lastChild);
            elt.removeChild(elt.lastChild);
        }
    },

    get_file_as_text: async function (file) {
        const reader = new FileReader(file);
        
        const p = new Promise((resolve, reject) => {
            reader.addEventListener("load", (evt) => {
                resolve(evt.target.result);
            });
            reader.addEventListener("error", (evt) => {
                reject(evt);
            });
        });

        reader.readAsText(file);
        return p;
    },

    date_re: /^[^T]+/,

    iso2date: function (isostr) {
        const full =`${isostr}T17:00:00`;
        return new Date(full);
    },

    date2iso: function (date) {
        return date.toISOString().match(CAL.date_re)[0];
    },

    text_td: function(text) {
        const td = document.createElement("td");
        td.appendChild(document.createTextNode(text));
        return td;
    },

    text_th: function(text) {
        const th = document.createElement("th");
        th.appendChild(document.createTextNode(text));
        return th;
    },

    label: function(text, elt) {
        const lab = document.createElement("label");
        lab.appendChild(document.createTextNode(text));
        if (typeof(elt) == "string") {
            lab.setAttribute("for", elt);
            return lab;
        } else if (elt.tagName) {
            elt.appendChild(lab);
        } else {
            return lab;
        }
    },

    barf_about_old_browsers: function() {
        if(typeof(document.createElement("dialog").showModal) == "function") {
            /* You're golden! */
        } else {
            /* You're brownish-yellow. */
            document.getElementById("old-browser").style.display = "block";
        }
    },
}

UTIL.set_text = function(elt, text) {
    UTIL.clear(elt);
    elt.appendChild(document.createTextNode(text));
}

async function are_you_sure(question) {
    UTIL.set_text(DISPLAY.confirm_message, question);
    DISPLAY.confirm.showModal();
    const p = new Promise((resolve, _) => {
        DISPLAY.confirm.onclose = () => {
            if(DISPLAY.confirm.returnValue == "ok") {
                resolve(true);
            } else {
                resolve(false);
            }
        }
    });
    return p;
}

const RQ = {
    id: 0,
    pending: new Map(),
    progress_div: document.querySelector("div#progress"),
    progress_list: document.querySelector("div#progress > ul"),
    error_div: document.querySelector("div#error"),
    error_list: document.querySelector("div#error > ul"),
    error_dismiss: document.querySelector("div#error > button"),
};
RQ.next_id = function() {
    const id = RQ.id;
    RQ.id = RQ.id + 1;
    return String(id);
}
RQ.add_pending = function(id, description) {
    RQ.pending.set(id, Date.now());
    const item = document.createElement("li");
    item.setAttribute("data-id", id);
    item.appendChild(document.createTextNode(description));
    RQ.progress_list.appendChild(item);
    RQ.progress_div.style.display = "flex";
}
RQ.remove_pending = function(id) {
    const item = RQ.progress_list.querySelector(`li[data-id="${id}"]`);
    if(item) {
        RQ.progress_list.removeChild(item);
    } else {
        console.log("No progress <LI> with data-id:", id);
    }
    RQ.pending.delete(id);
    if(RQ.pending.size == 0) {
        RQ.progress_div.style.display = "none";
    }
}
RQ.add_err = function(err) {
    const item = document.createElement("li");
    item.appendChild(document.createTextNode(err));
    RQ.error_list.appendChild(item);
    RQ.error_div.style.display = "flex";
}
RQ.error_dismiss.addEventListener("click",
    function() {
        RQ.error_div.style.display = "none";
        recursive_clear(RQ.error_list);
    }
);

function api_request(req, description, on_success) {
    const rq_id = RQ.next_id();

    req.headers.set("x-camp-request-id", rq_id);
    // The AUTH object should be defined in a <SCRIPT> tag in the HTML template.
    req.headers.set("x-camp-uname", AUTH.uname);
    req.headers.set("x-camp-key", AUTH.key);

    RQ.add_pending(rq_id, description);
    fetch(req)
    .then(r => {
        console.log("api_request() returned result:", r);
        on_success(r)
    })
    .catch(console.log)
    .finally(x => RQ.remove_pending(rq_id));
}



UTIL.ensure_on_load(UTIL.barf_about_old_browsers);