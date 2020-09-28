import yaml from 'js-yaml';
import { reactive } from 'vue';

export const state = reactive({
    entries: [],
    visible: false,
    instance: null,
});

function on_hook() {
    config_toggle(1);
    state.visible = true;
}

function on_config(raw, links) {
    let config = yaml.safeLoad(raw);
    state.entries = [];

    if (Array.isArray(config)) {
        for (let entry of config) {
            state.entries.push(entry);
        }
    }

    for (let link of links) {
        state.entries.push(link);
    }
}

config_attach(on_config, on_hook);

window.addEventListener('blur', e => hide(true));

window.addEventListener('focus', e => {
    state.instance.$refs.input.focus();
});

window.addEventListener('keydown', e => {
    if (e.code == "Escape") hide(true);

    if (e.code == "ArrowUp") {
        e.preventDefault();
        state.instance.select(-1);
    }

    if (e.code == "ArrowDown") {
        e.preventDefault();
        state.instance.select(1);
    }
});

export function hide(restore, callback) {
    if (state.visible) {
        state.visible = false;
        setTimeout(() => {
            config_toggle(restore ? 2 : 0);
            state.instance.reset();
            callback && callback()
        }, 200);
    }
}

export function entry_match(search, entry) {
    if ('names' in entry) {
        for (let value of entry.names)
            if (value.toLowerCase().startsWith(search))
                return [0, value.length];
    } else {
        if (entry.name.toLowerCase().startsWith(search))
            return [0, value.length];
    }

    return null;
    // let i = value.toLowerCase().indexOf(search);
    // if (i == -1) return null;
    // return [i, value.length];
}

export function entry_display(entry) {
    if ('names' in entry) {
        return entry.names[0];
    } else {
        return entry.name;
    }
}
