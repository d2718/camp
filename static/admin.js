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

const DISPLAY = {
    admin_tbody: document.querySelector("table#admin-table > tbody"),
}

function text_td(text) {
    const td = document.createElement("td");
    td.appendChild(document.createTextNode(text));
    return td;
}

function create_admin_actions_dropdown(uname) {
    const options = [
        ["email", "change email address"],
        ["delete", "delete user"]
    ];

    const input = document.createElement("select");
    input.setAttribute("list", "admin-action-list");
    input.setAttribute("data-uname", uname);
    for (const kvp of options) {
        const opt = document.createElement("option");
        opt.setAttribute("value", kvp[0]);
        opt.innerText = kvp[1];
        input.appendChild(opt);
    }
    /*
    TODO: add event handler to perform this action.
    */
    return input;
}

function add_user_to_display(u) {
    console.log("adding user to display:", u);

    if (u.Admin) {
        const v = u.Admin;
        const tr = document.createElement("tr");
        tr.setAttribute("data-uname", v.uname);
        tr.appendChild(text_td(v.uname));
        tr.appendChild(text_td(v.email));
        const td = document.createElement("td");
        td.appendChild(
            create_admin_actions_dropdown(v.uname)
        );
        tr.appendChild(td);

        DISPLAY.admin_tbody.appendChild(tr);
    }
}


function populate_admins(r) {
    r.json()
    .then(j => {
        console.log("populate-admins response:")
        console.log(j);

        recursive_clear(DISPLAY.admin_tbody);
        for (const u of j) {
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

    } else if(action == "populate-admins") {
        populate_admins(r);

    } else {
        const e_n = STATE.next_error();
        const err_txt = `Unrecognized x-camp-action header: ${action}. (See console error #${e_n})`;
        console.log(e_n, r);
        RQ.add_err(err_txt);
    }
}

function request_action(action, description) {
    const r = new Request(
        API_ENDPOINT,
        { 
            method: "POST",
            headers: { "x-camp-action": action }
        },
    );

    const desc = (description || action);

    api_request(r, desc, field_response);
}

function populate_all(_evt) {
    request_action("populate-admins", "Populating Admins...");
}

if(document.readyState == "complete") {
    populate_all(null);
} else {
    window.addEventListener("load", populate_all);
}
