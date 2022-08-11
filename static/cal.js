/*
Oh, man, we're implementing a calendar.
*/
const CAL = {
    dates: new Set(),
    include_on_drag: true,
    has_populated: false,
    target_div: document.getElementById("calendar-display"),
    year_selector: document.getElementById("cal-year"),
    month_names: {
        0: "Jan",
        1: "Feb",
        2: "Mar",
        3: "Apr",
        4: "May",
        5: "Jun",
        6: "Jul",
        7: "Aug",
        8: "Sep",
        9: "Oct",
        10: "Nov",
        11: "Dec",
    },
    current_academic_year: function() {
        const today = new Date();
        if(today.getMonth() < 7) {
            return today.getFullYear() - 1;
        } else {
            return today.getFullYear();
        }
    }
};
CAL.toggle_on_mousedown = function() {
    const date = this.getAttribute("data-date");
    if(CAL.dates.delete(date)) {
        this.removeAttribute("class");
        CAL.include_on_drag = false;
    } else {
        this.setAttribute("class", "y");
        CAL.dates.add(date);
        CAL.include_on_drag = true;
    }
}
CAL.set_on_drag = function(evt) {
    if(evt.buttons == 1) {
        const date = this.getAttribute("data-date");
        if(CAL.include_on_drag) {
            CAL.dates.add(date);
            this.setAttribute("class", "y");
        } else {
            CAL.dates.delete(date);
            this.removeAttribute("class");
        }
    }
}

CAL.make_month = function(year, month) {
    const tab = document.createElement("table");
    tab.setAttribute("class", "calendar-month");

    const thead = document.createElement("thead");

    const title_row = document.createElement("tr");
    const title_th = document.createElement("th");
    title_th.setAttribute("colspan", "7");
    title_th.appendChild(document.createTextNode(
        `${CAL.month_names[month]} ${year}`
    ));
    title_row.appendChild(title_th);
    thead.appendChild(title_row);

    const thr = document.createElement("tr");
    for(const d of ['S', 'M', 'T', 'W', 'R', 'F', 'S']) {
        const th = document.createElement("th");
        th.appendChild(document.createTextNode(d));
        thr.appendChild(th);
    }
    thead.appendChild(thr);
    tab.appendChild(thead);

    const tbody = document.createElement("tbody");

    const first_day = new Date(year, month, 1);
    let current_tr = null;
    if(first_day.getDay() > 0) {
        current_tr =  document.createElement("tr");
        for(let n = 0; n < first_day.getDay(); n++) {
            current_tr.appendChild(document.createElement("td"));
        }
    }
    let day_n = 1;
    let current_day = new Date(year, month, day_n);
    while(current_day.getMonth() == month) {
        if(current_day.getDay() == 0) {
            current_tr = document.createElement("tr");
        }
        const td = document.createElement("td");
        td.setAttribute("data-date", current_day);
        td.addEventListener("mousedown", CAL.toggle_on_mousedown);
        td.addEventListener("mouseover", CAL.set_on_drag);
        td.appendChild(document.createTextNode(current_day.getDate()));
        current_tr.appendChild(td);
        if(current_day.getDay() == 6) {
            tbody.appendChild(current_tr);
            current_tr = null;
        }
        day_n = day_n + 1;
        current_day = new Date(year, month, day_n);
    }
    if(current_tr) {
        tbody.appendChild(current_tr);
        current_tr = null;
    }

    tab.appendChild(tbody);
    return tab;
}

CAL.populate_year = function(target_elt, year) {
    recursive_clear(target_elt);

    for(let m = 7; m < 12; m++) {
        target_elt.appendChild(
            CAL.make_month(year, m)
        );
    }
    for(let m = 0; m < 7; m++) {
        target_elt.appendChild(
            CAL.make_month(year+1, m)
        );
    }
}

document.getElementById("cal-prev-year")
    .addEventListener("click", () => {
        const new_year = Number(CAL.year_selector.value) - 1;
        CAL.year_selector.value = new_year;
        CAL.populate_year(CAL.target_div, new_year);
    })
document.getElementById("cal-next-year")
    .addEventListener("click", () => {
        const new_year = Number(CAL.year_selector.value) + 1;
        CAL.year_selector.value = new_year;
        CAL.populate_year(CAL.target_div, new_year);
    })
CAL.year_selector.addEventListener("change", function(evt) {
    CAL.populate_year(CAL.target_div, Number(this.value));
})
document.getElementById("cal-tab-radio")
    .addEventListener("change", () => {
        if(!CAL.has_populated) {
            const cur_year = CAL.current_academic_year();
            CAL.year_selector.value = cur_year;
            CAL.populate_year(CAL.target_div, cur_year);
            CAL.has_populated = true;
        }
});