import yaml from 'js-yaml';
import { markRaw, reactive } from 'vue';

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
    state.entries = raw.map(entry => markRaw(entry));
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
    for (let key of entry.keys) {
        if (key.toLowerCase().startsWith(search))
            return key;
    }

    return null;
}
