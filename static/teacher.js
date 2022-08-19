/*
JS BS for Teacher interaction.
*/
"use strict";

const API_ENDPOINT = "/teacher";
const DATA = {
    courses: new Map(),
    chapters: new Map(),
    paces: new Map(),
    goals: new Map(),
};
const DISPLAY = {
    calbox: document.getElementById("cals"),
    goal_edit: document.getElementById("edit-goal"),
    course_input: document.getElementById("edit-goal-course"),
    seq_input: document.getElementById("edit-goal-seq"),
};

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

function interpret_score(str) {
    const [n, d] = str.split("/").map(x => Number(x));
    if(!n) {
        return null;
    } else if(d) {
        return n / d;
    } else if(n > 2) {
        return n / 100;
    } else {
        return n;
    }
}
function score2pct(str) {
    const p = interpret_score(str);
    if(p) {
        const pct = (100 * p).round();
        return `${pct}%`;
    }
}

const PCAL_COLS = ["course", "chapter", "due", "done", "tries", "score", "edit"];

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

    const ctd = UTIL.text_td(crs.title);
    ctd.setAttribute("title", crs.book);
    tr.appendChild(ctd);
    const chtd = UTIL.text_td(chp.title)
    if(chp.subject) { chtd.setAttribute("title", chp.subject); }
    tr.appendChild(chtd);
    const duetd = UTIL.text_td(g.due || "")
    duetd.setAttribute("class", "due");
    tr.appendChild(duetd);
    const donetd = UTIL.text_td(g.done || "");
    tr.appendChild(donetd);
    const triestd = UTIL.text_td(g.tries || "")
    triestd.setAttribute("class", "tries");
    tr.appendChild(triestd);
    const scoretd = document.createElement("td");
    if(g.score) {
        UTIL.set_text(`${g.score} (${score2pct(g.score)})`);
    }
    scoretd.setAttribute("class", "score");
    tr.appendChild(scoretd);

    const etd = document.createElement("td");
    etd.setAttribute("class", "edit");
    const complete = document.createElement("button");
    complete.setAttribute("data-id", g.id);
    complete.setAttribute("title", "complete goal");
    UTIL.label("\u2713", complete);
    etd.appendChild(complete);
    const edit = document.createElement("button");
    edit.setAttribute("data-id", g.id);
    edit.setAttribute("title", "edit goal");
    UTIL.label("\u270e", edit);
    edit.addEventListener("click", edit_goal);
    etd.appendChild(edit);
    tr.appendChild(etd);

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
    name = document.createElement("kbd");
    name.setAttribute("class", "uname");
    UTIL.set_text(name, cal.uname);
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

    const more_tr = document.createElement("tr");
    const more_td = document.createElement("td");
    more_tr.appendChild(more_td);
    more_td.setAttribute("colspan", PCAL_COLS.length);
    const more_div = document.createElement("div");
    more_div.setAttribute("class", "fullwidth");
    more_td.appendChild(more_div);

    const expbutt = document.createElement("button");
    expbutt.setAttribute("data-uname", cal.uname);
    UTIL.label("+ more +", expbutt);
    more_div.appendChild(expbutt);
    const addbutt = document.createElement("button");
    addbutt.setAttribute("data-uname", cal.uname);
    UTIL.label("add goal \u229e", addbutt);
    addbutt.addEventListener("click", edit_goal);
    more_div.appendChild(addbutt);

    tbody.appendChild(more_tr);

    return tab;
}

function populate_courses(r) {
    r.json()
    .then(j => {
        console.log("populate-courses response:", j);

        DATA.courses = new Map();
        DATA.chapters = new Map();
        const list = document.getElementById("course-names");
        UTIL.clear(list);

        for(const crs of j) {
            let chaps = new Array();
            for(const chp of crs.chapters) {
                DATA.chapters.set(chp.id, chp);
                chaps[chp.seq] = chp.id;
            }
            crs.chapters = chaps;
            DATA.courses.set(crs.sym, crs);

            let book_text = "";
            if(crs.book) { book_text = ` (${crs.book})`; }
            const option_text = `${crs.sym}: ${crs.title}${book_text}`;
            const opt = document.createElement("option");
            opt.value = crs.sym;
            UTIL.set_text(opt, option_text);
            list.appendChild(opt);
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
        DATA.goals = new Map();
        UTIL.clear(DISPLAY.calbox);

        for(const p of j) {
            DATA.paces.set(p.uname, p);
            for(const g of p.goals) {
                DATA.goals.set(g.id, g);
            }

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

/*


Now we get to the editing.


*/

function populate_seq_list(evt) {
    const list = document.getElementById("course-seqs");
    const sym = document.forms["edit-goal"].elements["course"].value;
    const crs = DATA.courses.get(sym);

    UTIL.clear(list);
    const seqs = crs.chapters.filter(x => Boolean(x))
        .map(id => DATA.chapters.get(id).seq);
    for(const n in seqs) {
        const opt = document.createElement("option");
        opt.value = n;
        list.appendChild(opt);
    }
    console.log(seqs);

    const min = Math.min.apply(null, seqs);
    const max = Math.max.apply(null, seqs);

    DISPLAY.seq_input.setAttribute("min", min);
    DISPLAY.seq_input.setAttribute("max", max);

    let book_text = "";
    if (crs.book) { book_text = ` (${crs.book})`; }
    const course_text = `${crs.sym}: ${crs.title}${book_text}`;
    DISPLAY.course_input.setAttribute("title", course_text);
}

document.getElementById("edit-goal-course")
    .addEventListener("change", populate_seq_list);

function edit_goal(evt) {
    const form = document.forms["edit-goal"];
    const del = document.getElementById("delete-goal");
    let id = this.getAttribute("data-id");
    const confirm = document.getElementById("edit-goal-confirm");

    if(id) {
        id = Number(id);
        const g = DATA.goals.get(id);
        form.elements["id"].value = id;
        form.elements["course"].value = g.sym;
        form.elements["seq"].value = g.seq;
        form.elements["due"].value = g.due;
        form.elements["review"].checked = g.rev;
        form.elements["incomplete"].checked = g.inc;
        del.disabled = false;
        populate_seq_list();
        confirm.removeAttribute("data-uname");
    } else {
        for(const ipt of form.elements) {
            if(ipt.value) { ipt.value = null; }
            if(ipt.checked) { ipt.checked = false; }
        }
        del.disabled = true;
        const uname = this.getAttribute("data-uname");
        confirm.setAttribute("data-uname", uname);
    }

    DISPLAY.goal_edit.showModal();
}



function edit_goal_submit(evt) {
    const form = document.forms["edit-goal"];
    const uname = this.getAttribute("data-uname") || "";
    const id = Number(form.elements["id"].value) || 0;
    const sym = form.elements["course"].value?.trim() || "";
    const course = DATA.courses.get(sym);
    const seq = Number(form.elements["seq"].value) || 0;
    const chapt = course.chapters[seq];
    if(sym == "") {
        RQ,add_err("You must select a valid course.");
        return;
    } else if(!course) {
        RQ.add_err(`"${sym} is not a valid course symbol.`);
        return;
    }
    if(!chapt) {
        const err = `You must select a valid chapter number for course "${sym}": ${course.title} (${course.book}).`
        RQ.add_err(err);
        return;
    }

    // Pre-fill default values for a new goal.
    let g = {
        "uname": uname,
        "done": null,
        "tries": null,
        "weight": 0,
        "score": null,
    };

    // If this is an extant goal, pre-fill all the extant data.
    if(form.elements["id"].value) {
        for(const [k, v] of Object.entries(DATA.goals.get(id))) {
            g[k] = v;
        }
    }

    g["id"] = id;
    g["sym"] = sym;
    g["seq"] = seq;
    g["rev"] = form.elements["review"].checked;
    g["inc"] = form.elements["incomplete"].checked;
    g["due"] = form.elements["due"].value || null;

    DISPLAY.goal_edit.closest();
    if(form.elements["id"].value) {
        request_action("update-goal", g, `Updating Goal ${id}`);
    } else {
        request_action("add-goal", g, `Adding new Goal: ${sym}, ${seq} for ${uname}`);
    }
    
}

document.getElementById("edit-goal-cancel")
    .addEventListener("click", (evt) => {
        evt.preventDefault(),
        DISPLAY.goal_edit.closest();
    });
document.getElementById("edit-goal-confirm")
    .addEventListener("click", edit_goal_submit);