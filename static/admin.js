/*
admin.js

Frontend JS BS to make the admin's page work.
*/
const API_ENDPOINT = "/admin";
const STATE = {
    error_count: 0
};
STATE.next_error = function() {
    const err = STATE.error_count;
    STATE.error_count += 1;
    return err;
}
const DATA = {
    users: new Map(),
};

const DISPLAY = {
    confirm: document.getElementById("are-you-sure"),
    confirm_message: document.querySelector("dialog#are-you-sure > p"),
    admin_tbody: document.querySelector("table#admin-table > tbody"),
    admin_edit:  document.getElementById("alter-admin"),
    boss_tbody:  document.querySelector("table#boss-table > tbody"),
    boss_edit:   document.getElementById("alter-boss"),
    teacher_tbody: null,
    student_tbody: null,
};

function set_text(elt, text) {
    recursive_clear(elt);
    elt.appendChild(document.createTextNode(text));
}

function text_td(text) {
    const td = document.createElement("td");
    td.appendChild(document.createTextNode(text));
    return td;
}
function label(text, elt) {
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
}

async function are_you_sure(question) {
    set_text(DISPLAY.confirm_message, question);
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

/*
Add user object to appropriate table. Also insert into the
DATA.users Map.
*/
function add_user_to_display(u) {
    console.log("adding user to display:", u);

    if (u.Admin) {
        const v = u.Admin;
        DATA.users.set(v.uname, u);

        const tr = document.createElement("tr");
        tr.setAttribute("data-uname", v.uname);
        tr.appendChild(text_td(v.uname));
        tr.appendChild(text_td(v.email));
        const td = document.createElement("td");
        const ebutt = document.createElement("button");
        label("edit", ebutt);
        ebutt.setAttribute("data-uname", v.uname);
        ebutt.addEventListener("click", edit_admin);
        td.appendChild(ebutt);
        tr.appendChild(td);

        DISPLAY.admin_tbody.appendChild(tr);
    } else if(u.Boss) {
        const v = u.Boss;
        DATA.users.set(v.uname, u);

        const tr = document.createElement("tr");
        tr.setAttribute("data-uname", v.uname);
        tr.appendChild(text_td(v.uname));
        tr.appendChild(text_td(v.email));
        const td = document.createElement("button");
        const ebutt = document.createElement("button");
        label("edit", ebutt);
        ebutt.setAttribute("data-uname", v.uname);
        ebutt.addEventListener("click", edit_boss);
        td.appendChild(ebutt);
        tr.appendChild(td);

        DISPLAY.boss_tbody.appendChild(tr);
    } else {
        console.log("add_user_to_display() not implemented for", u);
    }
}

function populate_users(r) {
    r.json()
    .then(j => {
        console.log("populate-users response:")
        console.log(j);

        // Iterate through users once to determine which types are included
        // and thus which user tables need clearing.
/*         const repopulate = new Set();
        for (const u of j) {
            if(u.Admin)        { repopulate.add(DISPLAY.admin_tbody); }
            else if(u.Boss)    { repopulate.add(DISPLAY.boss_tbody); }
            else if(u.Teacher) { repopulate.add(DISPLAY.teacher_tbody); }
            else if(u.Student) { repopulate.add(DISPLAY.student_tbody); }
            else { console.log ("User has unknown type:", u); }
        }
        for(const elt of repopulate) {
            recursive_clear(elt);
        } */

        DATA.users = new Map();
        recursive_clear(DISPLAY.admin_tbody);
        recursive_clear(DISPLAY.boss_tbody);
        //recursive_clear(DISPLAY.teacher_tbody);
        //recursive_clear(DISPLAY.student_tbody);
        for(const u of j) {
            add_user_to_display(u);
        }
    }).catch(RQ.add_err);
}

function field_response(r) {
    if(!r.ok) {
        r.text()
        .then(t => {
            const err_txt = `${t} (${r.status}: ${r.statusText})`;
            RQ.add_err(err_txt);
        }
        ).catch(e => {
            const e_n = STATE.next_error();
            const err_txt = `Error #${e_n} (see console)`;
            console.log(e_n, e, r);
            RQ.add_err(err_txt);
        })

        return;
    }

    let action = r.headers.get("x-camp-action");

    if (!action) {
        const e_n = STATE.next_error();
        const err_txt = `Response lacked x-camp-action header. (See console error #${e_n}.)`;
        console.log(e_n, r);
        RQ.add_err(err_txt);

    } else if(action == "populate-users") {
        populate_users(r);

    } else {
        const e_n = STATE.next_error();
        const err_txt = `Unrecognized x-camp-action header: ${action}. (See console error #${e_n})`;
        console.log(e_n, r);
        RQ.add_err(err_txt);
    }
}

function request_action(action, body, description) {
    const options = {
        method: "POST",
        headers: { "x-camp-action": action }
    };
    if(body) {
        const bt = typeof(body);
        if(bt == "string") {
            options.headers["content-type"] = "text/plain";
            options.body = body;
        } else if(bt == "object") {
            options.headers["content-type"] = "application/json"
            options.body = JSON.stringify(body);
        }
    }

    const r = new Request(
        API_ENDPOINT,
        options
    );

    const desc = (description || action);

    api_request(r, desc, field_response);
}

/*

PAGE LOAD SECTION

*/

function populate_all(_evt) {
    request_action("populate-users", "Populating Admins...");
}

if(document.readyState == "complete") {
    populate_all(null);
} else {
    window.addEventListener("load", populate_all);
}


/*

MAKING ACTUAL CHANGES SECTION

*/

/*
For editing current or adding new Admins.

If editing a current Admin, the `uname` input will be disabled.
This both prevents the uname from being changed (unames should
never be changed) and also signals the difference between adding
new and updating existing users.
*/
function edit_admin(evt) {
    const uname = this.getAttribute("data-uname");
    const form = document.forms['alter-admin'];
    document.getElementById("delete-admin")
        .setAttribute("data-uname", uname);

    if(uname) {
        const u = DATA.users.get(uname)['Admin'];
        form.elements['uname'].value = u.uname;
        form.elements['uname'].disabled = true;
        form.elements['email'].value = u.email;
    } else {
        form.elements['uname'].disabled = false;
        for(const ipt of form.elements) {
            ipt.value = "";
        }
    }

    DISPLAY.admin_edit.showModal();
}

// We add this functionality to the "add Admin" button.
document.getElementById("add-admin").addEventListener("click", edit_admin);

/*
Performs some cursory validation and submits the updated Admin info to
the server.

Requests either "update-user" or "add-user" depending on whether the
`uname` input in the `alter-admin` form is diabled or not.

Will throw an error and prevent the dialog from closing if
form data pseudovalidation fails.
*/
function edit_admin_submit() {
    const form = document.forms['alter-admin'];
    const data = new FormData(form);
    /*  The FormData() constructor skips disabled inputs, so we need to
        manually ensure the `uname` value is in there. */
    const uname_input = form.elements['uname'];
    data.set("uname", uname_input.value);

    const uname = data.get("uname") || "";
    let email = data.get("email") || "";
    email = email.trim();

    const u = {
        "Admin": {
            "uname": uname,
            "email": email,
            "role": "Admin",
            "salt": "",
        }
    };

    DISPLAY.admin_edit.close();
    if(uname_input.disabled) {
        request_action("update-user", u, `Updating user ${uname}...`);
    } else {
        request_action("add-user", u, `Adding user ${uname}...`);
    }
}

// The "cancel" <button> should close the dialog but not try to submit the form.
document.getElementById("alter-admin-cancel")
    .addEventListener("click", (evt) => {
        evt.preventDefault();
        DISPLAY.admin_edit.close();
    });
document.getElementById("alter-admin-confirm")
    .addEventListener("click", edit_admin_submit);

async function delete_admin_submit(evt) {
    const uname = this.getAttribute("data-uname");
    const q = `Are you sure you want to delete Admin ${uname}?`;
    if(await are_you_sure(q)) {
        DISPLAY.admin_edit.close();
        request_action("delete-user", uname, `Deleting ${uname}...`);
    }
}

document.getElementById("delete-admin")
    .addEventListener("click", delete_admin_submit)


/*
For editing current or adding new Bosses.

(Much of what follows is essential identical to the above section
on adding/altering Admins.)

If editing a current Boss, the `uname` input will be disabled.
This both prevents the uname from being changed (unames should
never be changed) and also signals the difference between adding
new and updating existing users.
*/
function edit_boss(evt) {
    const uname = this.getAttribute("data-uname");
    const form = document.forms['alter-boss'];
    document.getElementById("delete-boss")
        .setAttribute("data-uname", uname);

    if(uname) {
        const u = DATA.users.get(uname)['Boss'];
        form.elements['uname'].value = u.uname;
        form.elements['uname'].disabled = true;
        form.elements['email'].value = u.email;
    } else {
        form.elements['uname'].disabled = false;
        for(const ipt of form.elements) {
            ipt.value = "";
        }
    }

    DISPLAY.boss_edit.showModal();
}

// We add this functionality to the "add Admin" button.
document.getElementById("add-boss").addEventListener("click", edit_boss);

/*
Performs some cursory validation and submits the updated Admin info to
the server.

Requests either "update-user" or "add-user" depending on whether the
`uname` input in the `alter-admin` form is diabled or not.

Will throw an error and prevent the dialog from closing if
form data pseudovalidation fails.
*/
function edit_boss_submit() {
    const form = document.forms['alter-boss'];
    const data = new FormData(form);
    /*  The FormData() constructor skips disabled inputs, so we need to
        manually ensure the `uname` value is in there. */
    const uname_input = form.elements['uname'];
    data.set("uname", uname_input.value);

    const uname = data.get("uname") || "";
    let email = data.get("email") || "";
    email = email.trim();

    const u = {
        "Boss": {
            "uname": uname,
            "email": email,
            "role": "Boss",
            "salt": "",
        }
    };

    DISPLAY.boss_edit.close();
    if(uname_input.disabled) {
        request_action("update-user", u, `Updating user ${uname}...`);
    } else {
        request_action("add-user", u, `Adding user ${uname}...`);
    }
}

// The "cancel" <button> should close the dialog but not try to submit the form.
document.getElementById("alter-boss-cancel")
    .addEventListener("click", (evt) => {
        evt.preventDefault();
        DISPLAY.boss_edit.close();
    });
document.getElementById("alter-boss-confirm")
    .addEventListener("click", edit_boss_submit);

async function delete_boss_submit(evt) {
    const uname = this.getAttribute("data-uname");
    const q = `Are you sure you want to delete Boss ${uname}?`;
    if(await are_you_sure(q)) {
        DISPLAY.boss_edit.close();
        request_action("delete-user", uname, `Deleting ${uname}...`);
    }
}

document.getElementById("delete-boss")
    .addEventListener("click", delete_boss_submit);