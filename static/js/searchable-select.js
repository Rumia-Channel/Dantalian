function createSearchableSelect(container, opts) {
    const {
        options = [],
        value = null,
        placeholder = "選択...",
        onChange = null,
        searchable = true,
        clearable = true,
    } = opts;

    let selectedValue = value;
    let selectedLabel = "";
    let isOpen = false;

    const selected = options.find((o) => o.value === value);
    if (selected) selectedLabel = selected.label;

    const wrapper = document.createElement("div");
    wrapper.className = "ss";

    const control = document.createElement("div");
    control.className = "ss-control";

    const input = document.createElement("input");
    input.type = "text";
    input.className = "ss-input";
    input.placeholder = placeholder;
    input.value = selectedLabel;
    input.setAttribute("autocomplete", "off");

    const arrow = document.createElement("span");
    arrow.className = "ss-arrow";

    const clearBtn = document.createElement("span");
    clearBtn.className = "ss-clear";
    clearBtn.textContent = "\u00D7";
    clearBtn.hidden = !selectedValue;

    control.appendChild(input);
    control.appendChild(arrow);
    control.appendChild(clearBtn);

    const dropdown = document.createElement("div");
    dropdown.className = "ss-dropdown hidden";

    const list = document.createElement("div");
    list.className = "ss-list";

    const emptyMsg = document.createElement("div");
    emptyMsg.className = "ss-empty";
    emptyMsg.textContent = "該当なし";

    dropdown.appendChild(list);
    dropdown.appendChild(emptyMsg);

    wrapper.appendChild(control);
    wrapper.appendChild(dropdown);
    container.appendChild(wrapper);

    function render(query) {
        list.innerHTML = "";
        const q = (query || "").toLowerCase();
        let count = 0;

        if (!q) {
            for (const o of options) {
                list.appendChild(makeItem(o));
                count++;
            }
        } else {
            for (const o of options) {
                if (o.label.toLowerCase().includes(q) || String(o.value).toLowerCase().includes(q)) {
                    list.appendChild(makeItem(o));
                    count++;
                }
            }
        }

        emptyMsg.hidden = count > 0;
        list.classList.toggle("hidden", count === 0);
        dropdown.classList.toggle("hidden", !isOpen);
    }

    function makeItem(o) {
        const item = document.createElement("div");
        item.className = "ss-item" + (o.value === selectedValue ? " ss-item--active" : "");
        item.textContent = o.label;
        item.addEventListener("mousedown", (e) => {
            e.preventDefault();
            select(o.value, o.label);
        });
        return item;
    }

    function select(val, label) {
        selectedValue = val;
        selectedLabel = label;
        input.value = label;
        clearBtn.hidden = !val && val !== 0;
        close();
        if (onChange) onChange(val);
    }

    function open() {
        if (isOpen) return;
        isOpen = true;
        input.value = "";
        render("");
        input.focus();
    }

    function close() {
        isOpen = false;
        dropdown.classList.add("hidden");
        input.value = selectedLabel;
    }

    input.addEventListener("focus", () => {
        if (!isOpen) open();
    });

    input.addEventListener("input", () => {
        render(input.value);
    });

    input.addEventListener("blur", () => {
        setTimeout(close, 150);
    });

    input.addEventListener("keydown", (e) => {
        if (e.key === "Escape") {
            close();
            input.blur();
        } else if (e.key === "Enter") {
            const first = list.querySelector(".ss-item");
            if (first) first.dispatchEvent(new Event("mousedown"));
        }
    });

    arrow.addEventListener("mousedown", (e) => {
        e.preventDefault();
        if (isOpen) {
            close();
            input.blur();
        } else {
            input.focus();
        }
    });

    if (clearable) {
        clearBtn.addEventListener("mousedown", (e) => {
            e.preventDefault();
            select(null, "");
        });
    }

    return {
        wrapper,
        getValue: () => selectedValue,
        setValue: (val) => {
            const o = options.find((op) => op.value === val);
            if (o) {
                select(o.value, o.label);
            } else {
                select(null, "");
            }
        },
        updateOptions: (newOpts) => {
            opts.options = newOpts;
            const o = newOpts.find((op) => op.value === selectedValue);
            if (o) {
                selectedLabel = o.label;
                if (!isOpen) input.value = selectedLabel;
            } else {
                select(null, "");
            }
            if (isOpen) render(input.value);
        },
        destroy: () => wrapper.remove(),
    };
}

document.addEventListener("click", (e) => {
    if (!e.target.closest(".ss")) {
        document.querySelectorAll(".ss-dropdown:not(.hidden)").forEach((dd) => {
            dd.classList.add("hidden");
            const ss = dd.closest(".ss");
            if (ss) {
                const inp = ss.querySelector(".ss-input");
                const val = ss._ssValue;
                if (inp && val !== undefined) inp.value = val;
            }
        });
    }
});
