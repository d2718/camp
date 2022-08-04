/*
api_request.js

The machinery for tracking which API requests have come back and displaying
progress accordingly.
*/

const AUTH = {
    uname: "",
    key: ""
};

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
    RQ.pending.set(rq_id, Date.now());
    const item = document.createElement("li");
    item.setAttribute("data-id", id);
    item.appendChild(document.createTextNode(description));
    RQ.progress_list.appendChild(item);
    RQ.progress_div.style.display = "flex";
}
RQ.remove_pending(id) {
    const item = RQ.progress_list.querySelector(`li[data-id="${id}"]`);
    if(item) {
        RQ.progress_list.removeChild(item);
    } else {
        console.log("No progress <LI> with data-id:", id);
    }
    RQ.pending.delete(id);
    if(RQ.pending.size() == 0) {
        RQ.progress_div.style.display = "none";
    }
}

function request(uri, req_obj, description, on_success) {
    const rq_id = RQ.next_id();

    req_obj.headers["x-camp-request-id"] = rq_id;
    req_obj.headers["x-camp-uname"] = AUTH.uname;
    req_obj.headers["x-camp-key"] = AUTH.key;

    RQ.add_pending(id);
    fetch(uri, req_obj)
    .then(r => on_success(r))
    .catch(console.log)
    .finally(x => RQ.remove_pending(id));
}