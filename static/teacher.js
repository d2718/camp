/*
JS BS for Teacher interaction.
*/
"use strict";

const API_ENDPOINT = "/teacher";
const DATA = {
    courses: new Map(),
    chapters: new Map(),
    paces: new Map(),
};
const DISPLAY = {
    calbox: document.getElementById("cals"),
}

const NOW = new Date();

let next_err = function() {}
{
    let err_count = 0;
    next_err = function() {
        const err = err_count;
        err_count += 1;
        return err;
    }
}

function log_numbered_error(e) {
    const errno = next_err();
    const err_txt = `${e} (See console error #${errno}.)`;
    console.error(`Error #${errno}:`, e, e.stack);
    RQ.add_err(err_txt);
}

function ratio2pct(num, denom) {
    if(Math.abs(denom) < 0.0001) { return "0%"; }
    const pct = Math.round(100 * num / denom);
    return `${pct}%`;
}

const PCAL_COLS = ["course", "chapter", "due", "done", "tries", "score"];

function row_from_goal(g) {
    const crs = DATA.courses.get(g.sym);
    const chp = DATA.chapters.get(crs.chapters[g.seq]);

    const tr = document.createElement("tr");
    tr.setAttribute("data-id", g.id);
    let due = null;
    let done = null;
    if(chp.due) { due = UTIL.iso2date(chp.due); }
    if(chp.done) { done = UTIL.iso2date(chp.done); }
    if(due) {
        if(done) {
            if(due < done) {
                tr.setAttribute("class", "late");
            } else {
                tr.setAttribute("class", "done");
            }
        } else {
            if(due < NOW) {
                tr.setAttribute("class", "due");
            } else {
                tr.setAttribute("class", "yet");
            }
        }
    } else {
        if(done) {
            tr.setAttribute("class", "done");
        } else {
            tr.setAttribute("class", "yet");
        }
    }

    tr.appendChild(UTIL.text_td(crs.title));
    tr.appendChild(UTIL.text_td(chp.title));
    tr.appendChild(UTIL.text_td(chp.due || ""));
    tr.appendChild(UTIL.text_td(chp.done || ""));
    tr.appendChild(UTIL.text_td(chp.tries || ""));
    tr.appendChild(UTIL.text_td(chp.score || ""));

    return tr;
}

function make_calendar_table(cal) {
    const tab = document.createElement("table");
    tab.setAttribute("class", "pace");
    tab.setAttribute("data-uname", cal.uname);

    const thead = document.createElement("thead");
    tab.appendChild(thead);
    const sum_row = document.createElement("tr");
    const sum_td = document.createElement("td");
    sum_td.setAttribute("colspan", String(PCAL_COLS.length));
    const summary = document.createElement("div");
    summary.setAttribute("class", "summary");
    sum_td.appendChild(summary);
    sum_row.appendChild(sum_td);
    thead.appendChild(sum_row);
    {
        const tr = document.createElement("tr");
        for(const lab of PCAL_COLS) {
            tr.appendChild(UTIL.text_th(lab));
        }
        thead.appendChild(tr);
    }
    
    const tbody = document.createElement("tbody");
    tab.appendChild(tbody);

    let n_due = 0;
    let n_done = 0;

    for(const g of cal.goals) {
        tbody.appendChild(row_from_goal(g));
        if(g.done) {
            n_done += 1;
        }
        if(g.due) {
            let due = UTIL.iso2date(g.due);
            if(due < NOW) {
                n_due += 1;
            }
        }
    }

    const names = document.createElement("div");
    let name = document.createElement("span");
    name.setAttribute("class", "full");
    UTIL.set_text(name, `${cal.last}, ${cal.rest}`);
    names.appendChild(name);
    names.appendChild(document.createElement("br"));
    name = document.createElement("span");
    name.setAttribute("class", "uname");
    UTIL.set_text(name, `(${cal.uname})`);
    names.appendChild(name);
    summary.appendChild(names);

    const numbers = document.createElement("div");
    let lead_pct = ratio2pct(cal.done_weight - cal.due_weight, cal.total_weight);
    if(cal.done_weight >= cal.due_weight) {
        lead_pct = "+" + lead_pct;
    } else {
        numbers.setAttribute("class", "bad");
    }
    const num_txt = `done ${n_done} / ${n_due} due (${lead_pct})`;
    UTIL.set_text(numbers, num_txt);
    summary.appendChild(numbers);

    return tab;
}

function populate_courses(r) {
    r.json()
    .then(j => {
        console.log("populate-courses response:", j);

        DATA.courses = new Map();
        DATA.chapters = new Map();
        for(const crs of j) {
            let chaps = new Array();
            for(const chp of crs.chapters) {
                DATA.chapters.set(chp.id, chp);
                chaps[chp.seq] = chp.id;
            }
            crs.chapters = chaps;
            DATA.courses.set(crs.sym, crs);
        }

        request_action("populate-goals", "", "Populating pace calendars.")
    })
    .catch(log_numbered_error);
}

function populate_goals(r) {
    r.json()
    .then(j => {
        console.log("populate-goals response:", j);

        DATA.paces = new Map();
        UTIL.clear(DISPLAY.calbox);

        for(const p of j) {
            DATA.paces.set(p.uname, p);

            const tab = make_calendar_table(p);
            DISPLAY.calbox.appendChild(tab);
        }
    })
    .catch(log_numbered_error);
}

function field_response(r) {
    if(!r.ok) {
        r.text()
        .then(t => {
            const err_txt = `${t}\n(${r.status}: ${r.statusText})`;
            RQ.add_err(err_txt);
        }
        ).catch(e => {
            const e_n = next_err();
            const err_txt = `Error #${e_n} (see console)`;
            console.log(`Error #${e_n}:`, e, r);
            RQ.add_err(err_txt);
        });

        return;
    }

    let action = r.headers.get("x-camp-action");

    if(!action) {
        const e_n = next_err();
        const err_txt = `Response lacked x-camp-action header. (See console error #${e_n}.)`;
        console.log(`Error #${e_n} response:`, r);
        RQ.add_err(err_txt);

    } else if(action == "populate-courses") {
        populate_courses(r);
    } else if(action == "populate-goals") {
        populate_goals(r);
    } else {
        const e_n = next_err();
        const err_txt = `Unrecognized x-camp-action header: "${action}". (See console error #${e_n}.)`;
        console.log(`Error #${e_n} response:`, r);
        RQ.add_err(err_txt);
    }
}

function request_action(action, body, description) {
    const options = {
        method: "POST",
        headers: { "x-camp-action": action }
    };
    if(body) {
        const btype = typeof(body);
        if(btype == "string") {
            options.headers["content-type"] = "text/plain";
            options.body = body;
        } else if(btype == "object") {
            options.headers["content-type"] = "application/json";
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

UTIL.ensure_on_load(() => {
    request_action("populate-courses", "", "Fetching Course data.")
});