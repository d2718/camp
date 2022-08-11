/*
Oh, man, we're implementing a calendar.
*/

function make_month(year, month) {
    const tab = document.createElement("table");
    tab.setAttribute("class", "calendar-month");
    const thead = document.createElement("thead");
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
    for(let n = 0; n < first_day.getDay(); n++) {
        
    }

}