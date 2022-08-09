/*
admin.js

Frontend JS BS to make the admin's page work.

The util.js script must load before this one. It should be loaded
synchronously at the bottom of the <BODY>, and this should be
DEFERred.
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
    courses: new Map(),
};

const DISPLAY = {
    confirm: document.getElementById("are-you-sure"),
    confirm_message: document.querySelector("dialog#are-you-sure > p"),
    admin_tbody:   document.querySelector("table#admin-table > tbody"),
    admin_edit:    document.getElementById("alter-admin"),
    boss_tbody:    document.querySelector("table#boss-table > tbody"),
    boss_edit:     document.getElementById("alter-boss"),
    teacher_tbody: document.querySelector("table#teacher-table > tbody"),
    teacher_edit:  document.getElementById("alter-teacher"),
    student_tbody: document.querySelector("table#student-table > tbody"),
    student_edit:  document.getElementById("alter-student"),
    student_upload: document.getElementById("upload-students-dialog"),
    student_paste: document.getElementById("paste-students-dialog"),
    course_tbody:  document.querySelector("table#course-table > tbody"),
    course_upload: document.getElementById("upload-course-dialog"),
};

function populate_users(r) {
    r.json()
    .then(j => {
        console.log("populate-users response:")
        console.log(j);

        DATA.users = new Map();
        recursive_clear(DISPLAY.admin_tbody);
        recursive_clear(DISPLAY.boss_tbody);
        recursive_clear(DISPLAY.teacher_tbody);
        recursive_clear(DISPLAY.student_tbody);
        for(const u of j) {
            add_user_to_display(u);
        }
    }).catch(RQ.add_err);
}

function field_response(r) {
    if(!r.ok) {
        r.text()
        .then(t => {
            const err_txt = `${t}\n(${r.status}: ${r.statusText})`;
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
    } else if(action == "populate-courses") {
        populate_courses(r);
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

USERS section

The functions and objects in this section are for dealing with Users,
that is, the stuff on the "Staff" and "Students" tabs.

*/

function make_user_edit_button_td(uname, edit_func) {
    const butt = document.createElement("button");
    butt.setAttribute("data-uname", uname);
    label("edit", butt);
    butt.addEventListener("click", edit_func);
    const td = document.createElement("td");
    td.appendChild(butt);
    return td;
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
        tr.appendChild(make_user_edit_button_td(v.uname, edit_admin));

        DISPLAY.admin_tbody.appendChild(tr);

    } else if(u.Boss) {
        const v = u.Boss;
        DATA.users.set(v.uname, u);

        const tr = document.createElement("tr");
        tr.setAttribute("data-uname", v.uname);
        tr.appendChild(text_td(v.uname));
        tr.appendChild(text_td(v.email));
        tr.appendChild(make_user_edit_button_td(v.uname, edit_boss));

        DISPLAY.boss_tbody.appendChild(tr);

    } else if(u.Teacher) {
        const v = u.Teacher.base;
        DATA.users.set(v.uname, u);

        const tr = document.createElement("tr");
        tr.setAttribute("data-uname", v.uname);
        tr.appendChild(text_td(v.uname));
        tr.appendChild(text_td(v.email));
        tr.appendChild(text_td(u.Teacher.name));
        tr.appendChild(make_user_edit_button_td(v.uname, edit_teacher));

        DISPLAY.teacher_tbody.appendChild(tr);
    
    } else if(u.Student) {
        const v = u.Student.base;
        const s = u.Student;
        DATA.users.set(v.uname, u);

        const tr = document.createElement("tr");
        tr.setAttribute("data-uname", v.uname);
        tr.appendChild(text_td(v.uname));
        tr.appendChild(text_td(`${s.last}, ${s.rest}`));
        tr.appendChild(text_td(s.teacher));
        tr.appendChild(text_td(v.email));
        tr.appendChild(text_td(s.parent));
        tr.appendChild(make_user_edit_button_td(v.uname, edit_student));

        DISPLAY.student_tbody.appendChild(tr);

    } else {
        console.log("add_user_to_display() not implemented for", u);
    }
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
    const del = document.getElementById("delete-admin")
    del.setAttribute("data-uname", uname);

    if(uname) {
        const u = DATA.users.get(uname)['Admin'];
        form.elements['uname'].value = u.uname;
        form.elements['uname'].disabled = true;
        form.elements['email'].value = u.email;
        del.disabled = false
    } else {
        form.elements['uname'].disabled = false;
        for(const ipt of form.elements) {
            ipt.value = "";
        }
        del.disabled = true;
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
    const del = document.getElementById("delete-boss")
    del.setAttribute("data-uname", uname);

    if(uname) {
        const u = DATA.users.get(uname)['Boss'];
        form.elements['uname'].value = u.uname;
        form.elements['uname'].disabled = true;
        form.elements['email'].value = u.email;
        del.disabled = false;
    } else {
        form.elements['uname'].disabled = false;
        for(const ipt of form.elements) {
            ipt.value = "";
        }
        del.removeAttribute("data-uname");
        del.disabled = true;
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


function edit_teacher(evt) {
    const uname = this.getAttribute("data-uname");
    const form = document.forms['alter-teacher'];
    const del = document.getElementById("delete-teacher");
    del.setAttribute("data-uname", uname);
    
    if(uname) {
        const u = DATA.users.get(uname)['Teacher'];
        form.elements['uname'].value = u.base.uname;
        form.elements['uname'].disabled = true;
        form.elements['email'].value = u.base.email;
        form.elements['name'].value = u.name;
        del.disabled = false;
    } else {
        for(const ipt of form.elements) {
            ipt.value = "";
        }
        del.removeAttribute("data-uname");
        del.disabled = true;
    }

    DISPLAY.teacher_edit.showModal();
}

document.getElementById("add-teacher")
    .addEventListener("click", edit_teacher);

function edit_teacher_submit() {
    const form = document.forms['alter-teacher'];
    const data = new FormData(form);
    // Manually ensure possibly-disabled input value still here.
    const uname_input = form.elements["uname"];
    data.set("uname", uname_input.value);

    const uname = data.get("uname") || "";
    const email = (data.get("email") || "").trim();
    const name = (data.get("name") || "").trim();

    const u = {
        "Teacher": {
            "base": {
                "uname": uname,
                "role": "Teacher",
                "salt": "",
                "email": email,
            },
            "name": name
        }
    };

    DISPLAY.teacher_edit.close();
    if(uname_input.disabled) {
        request_action("update-user", u, `Updating user ${uname}...`);
    } else {
        request_action("add-user", u, `Adding user $[uname]...`);
    }
}

document.getElementById("alter-teacher-cancel")
    .addEventListener("click", (evt => {
        evt.preventDefault();
        DISPLAY.teacher_edit.close();
    }));
document.getElementById("alter-teacher-confirm")
    .addEventListener("click", edit_teacher_submit);

async function delete_teacher_submit(evt) {
    const uname = this.getAttribute("data-uname");
    const q = `Are you sure you want to delete Teacher ${uname}?`;
    if(await are_you_sure(q)) {
        DISPLAY.teacher_edit.close();
        request_action("delete-user", uname, `Deleting ${uname}...`);
    }
}

document.getElementById("delete-teacher")
    .addEventListener("click", delete_teacher_submit);

function populate_teacher_selector(teacher_uname) {
    let sel = document.getElementById("alter-student-teacher");
    
    recursive_clear(sel);

    for(const [uname, u] of DATA.users) {
        if(u.Teacher) {
            const opt = document.createElement("option");
            set_text(opt, u.Teacher.name);
            opt.value = uname;
            sel.appendChild(opt);
        }
    }

    if(teacher_uname) {
        sel.value = teacher_uname;
    }
}

function edit_student(evt) {
    const uname = this.getAttribute("data-uname");
    const form = document.forms["alter-student"];
    const del = document.getElementById("delete-student");
    del.setAttribute("data-uname", uname);
    
    if(uname) {
        const u = DATA.users.get(uname)['Student'];
        const b = u.base;

        form.elements["uname"].value = b.uname;
        form.elements["uname"].disabled = true;
        form.elements["last"].value = u.last;
        form.elements["rest"].value = u.rest;
        form.elements["email"].value = b.email;
        form.elements["parent"].value = u.parent;
        populate_teacher_selector(u.teacher);
        del.disabled = false;

    } else {
        form.elements["uname"].disabled = false;
        for(const ipt of form.elements) {
            ipt.value = "";
        }
        populate_teacher_selector(null);
        del.removeAttribute("data-uname");
        del.disabled = true;
    }

    DISPLAY.student_edit.showModal();
}

document.getElementById("add-student")
    .addEventListener("click", edit_student);

function edit_student_submit() {
    const form = document.forms['alter-student'];
    const data = new FormData(form);
    const uname_input = form.elements['uname'];
    data.set("uname", uname_input.value);

    const uname = data.get("uname") || "";
    const email = (data.get("email") || "").trim();
    const last = data.get("last") || "";
    const rest = data.get("rest") || "";
    const teacher = data.get("teacher");
    const parent = (data.get("parent") || "").trim();

    let u = {
        "Student": {
            "base": {
                "uname": uname,
                "role": "Student",
                "salt": "",
                "email": email,
            },
            "last": last,
            "rest": rest,
            "teacher": teacher,
            "parent": parent,
            "fall_exam_fraction": 0.2,
            "spring_exam_fraction": 0.2,
            "fall_notices": 0,
            "spring_notices": 0,
        }
    };

    console.log("Inserting student:", u);

    DISPLAY.student_edit.close();
    if(uname_input.disabled) {
        request_action("update-user", u, `Updating user $[uname]...`);
    } else {
        request_action("add-user", u, `Adding user ${uname}...`);
    }
}

document.getElementById("alter-student-cancel")
    .addEventListener("click", (evt)=> {
        evt.preventDefault();
        DISPLAY.student_edit.close();
    });
document.getElementById("alter-student-confirm")
    .addEventListener("click", edit_student_submit);

async function delete_student_submit(evt) {
    const uname = this.getAttribute("data-uname");
    const q = `Are you sure you want to delete Student ${uname}?`;
    if(await are_you_sure(q)) {
        DISPLAY.student_edit.close();
        request_action("delete-user", uname, `Deleting ${uname}...`);
    }
}

document.getElementById("delete-student")
    .addEventListener("click", delete_student_submit);


document.getElementById("paste-students")
    .addEventListener("click", () => {
        DISPLAY.student_paste.showModal(); 
    });

function paste_students_submit(evt) {
    const area = document.getElementById("student-csv-content");
    const data = area.value.trim();
    if(data == "") {
        RQ.add_err("Please enter some text before submitting.");
        return;
    }
    DISPLAY.student_paste.close();
    area.value = "";
    request_action("upload-students", data, `Uploading new students...`);
}

document.getElementById("paste-students-confirm")
    .addEventListener("click", paste_students_submit);


document.getElementById("upload-students")
    .addEventListener("click", () => {
        DISPLAY.student_upload.showModal();
    });

function upload_students_submit(evt) {
    const form = document.forms["upload-students"];
    const data = new FormData(form);
    const file = data.get("file");

    get_file_as_text(file)
    .then((text) => {
        DISPLAY.student_upload.close();
        request_action("upload-students", text, `Uploading new students...`);
    })
    .catch((err) => {
        RQ.add_err(`Error opening local file: ${err}`);
    })
}

document.getElementById("upload-students-confirm")
    .addEventListener("click",upload_students_submit);


/*

COURSES section

*/

function add_course_to_display(c) {
    console.log("adding course to display", c);

    const tr = document.createElement("tr");
    tr.setAttribute("data-sym", c.sym);
    tr.appendChild(text_td(c.sym));
    tr.appendChild(text_td(c.title));
    tr.appendChild(text_td(c.level));
    let td = document.createElement("td");
    const cite = document.createElement("cite");
    set_text(cite, c.book);
    td.appendChild(cite);
    tr.appendChild(td);
    tr.appendChild(text_td(c.chapters.length));

    DISPLAY.course_tbody.appendChild(tr);
}

function populate_courses(r) {
    r.json()
    .then(j => {
        console.log("populate-courses response:", j);

        DATA.courses = new Map();
        recursive_clear(DISPLAY.course_tbody);
        for(const c of j) {
            add_course_to_display(c);
        }
    }).catch(RQ.add_err);
}

document.getElementById("upload-course")
    .addEventListener("click", () => {
        DISPLAY.course_upload.showModal();
    });

function upload_course_submit(evt) {
    const form = document.forms["upload-course"];
    const data = new FormData(form);
    const file = data.get("file");

    get_file_as_text(file)
    .then((text) => {
        DISPLAY.course_upload.close();
        request_action("upload-course", text, `Uploading new students...`);
    })
    .catch((err) => {
        RQ.add_err(`Error opening local file: ${err}`);
    });
}

document.getElementById("upload-course-confirm")
    .addEventListener("click", upload_course_submit);

/*

PAGE LOAD SECTION

*/

console.log(DISPLAY);

ensure_on_load(() => {
    request_action("populate-users", "", "Fetching User data...");
    request_action("populate-courses", "", "Fetching Course data...");
});
