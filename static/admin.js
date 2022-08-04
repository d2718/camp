/*
admin.js

Frontend JS BS to make the admin's page work.
*/

const butt = document.getElementById("push-me");
butt.addEventListener("click",
    function(evt) {
        request(
            "/admin",
            {
                method: "POST",
                headers: {
                    "x-camp-action": "test"
                },
                body: "This is some body text."
            },
            "Trying a request...",
            r => {
                r.text().then(RQ.add_err)
                .catch(RQ.add_err);
            }
        );
    }
);