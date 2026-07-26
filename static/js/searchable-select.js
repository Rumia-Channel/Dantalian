function createSearchableSelect(container, opts) {
    if (opts.native) return createNativeSelect(container, opts);

    let {
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

    control.appendChild(input);
    control.appendChild(arrow);

    let clearBtn;
    if (clearable) {
        clearBtn = document.createElement("span");
        clearBtn.className = "ss-clear";
        clearBtn.textContent = "\u00D7";
        clearBtn.hidden = !selectedValue;
        control.appendChild(clearBtn);
    }

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
            e.stopPropagation();
            select(o.value, o.label);
        });
        return item;
    }

    function select(val, label) {
        selectedValue = val;
        selectedLabel = label || "";
        input.value = selectedLabel;
        if (clearBtn) clearBtn.hidden = val == null;
        isOpen = false;
        dropdown.classList.add("hidden");
        if (onChange) onChange(val);
    }

    function open() {
        if (isOpen) return;
        closeAllOpen();
        isOpen = true;
        input.value = "";
        render("");
        input.focus();
    }

    function close() {
        if (!isOpen) return;
        isOpen = false;
        dropdown.classList.add("hidden");
        input.value = selectedLabel;
    }

    wrapper._close = close;

    input.addEventListener("focus", () => {
        if (!isOpen) open();
    });

    input.addEventListener("input", () => {
        render(input.value);
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
        e.stopPropagation();
        if (isOpen) {
            close();
            input.blur();
        } else {
            open();
        }
    });

    if (clearable) {
        clearBtn.addEventListener("mousedown", (e) => {
            e.preventDefault();
            e.stopPropagation();
            select(null, "");
        });
    }

    dropdown.addEventListener("mousedown", (e) => {
        e.preventDefault();
    });

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
            options = newOpts;
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

function createNativeSelect(container, opts) {
    const {
        options = [],
        value = null,
        onChange = null,
        clearable = true,
        placeholder = null,
    } = opts;

    const select = document.createElement("select");
    select.className = "ss-native";

    if (clearable && placeholder) {
        const ph = document.createElement("option");
        ph.value = "";
        ph.textContent = placeholder;
        select.appendChild(ph);
    }

    function buildOptions(opts) {
        select.querySelectorAll("option[data-ss-opt]").forEach((el) => el.remove());
        for (const o of opts) {
            const opt = document.createElement("option");
            opt.value = o.value;
            opt.textContent = o.label;
            opt.dataset.ssOpt = "1";
            if (o.value == value) opt.selected = true;
            select.appendChild(opt);
        }
    }

    buildOptions(options);

    select.addEventListener("change", () => {
        const v = select.value === "" ? null : select.value;
        if (onChange) onChange(v);
    });

    container.appendChild(select);

    return {
        wrapper: select,
        getValue: () => {
            const v = select.value;
            return v === "" ? null : v;
        },
        setValue: (val) => {
            select.value = val == null ? "" : val;
        },
        updateOptions: (newOpts) => {
            buildOptions(newOpts);
        },
        destroy: () => select.remove(),
    };
}

function closeAllOpen() {
    document.querySelectorAll(".ss-dropdown:not(.hidden)").forEach((dd) => {
        const ss = dd.closest(".ss");
        if (ss && ss._close) ss._close();
    });
}

document.addEventListener("mousedown", (e) => {
    if (!e.target.closest(".ss")) {
        closeAllOpen();
    }
});
