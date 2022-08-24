
const SORTS = {
    "name": (a, b) => a.getAttribute("data-name").localeCompare(b.getAttribute("data-name")),
    "teacher": (a, b) => a.getAttribute("data-tname").localeCompare(b.getAttribute("data-tname")),
    "lag": (a, b) => Number(a.getAttribute("data-lag")) - Number(b.getAttribute("data-lag")),
};

function toggle_table_body(evt) {
    const tab = this.parentElement;
    const body = tab.querySelector("tbody");
    if(body.style.display == "table-row-group") {
        body.style.display = "none";
    } else {
        body.style.display = "table-row-group";
    }
}

function sort_tables(cmpfuncs) {
    const tab_arr = new Array();
    const cal_div = document.getElementById("cals");

    while(cal_div.firstChild) {
        tab_arr.push(cal_div.removeChild(cal_div.lastChild));
    }

    for(const f of cmpfuncs) {
        tab_arr.sort(f);
    }

    console.log(tab_arr);

    for(const tab of tab_arr) {
        cal_div.appendChild(tab);
    }
}

for(const tab of document.querySelectorAll("table")) {
    tab.querySelector("thead").addEventListener("click", toggle_table_body);
}

document.getElementById("name").addEventListener("click",
    () => sort_tables([SORTS.name])
);
document.getElementById("teacher").addEventListener("click",
    () => sort_tables([SORTS.name, SORTS.teacher])
);
document.getElementById("lag").addEventListener("click",
    () => sort_tables([SORTS.name, SORTS.lag])
);
