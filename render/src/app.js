import { markRaw, reactive, watchEffect } from 'vue';

export const state = reactive({
    search: '',
    matches: [],
    visible: false,
    instance: null,
});

search.hook(() => {
    console.log('on hook');
    search.toggle(1);
    state.visible = true;
});

watchEffect(() => {
    console.log(state.search);
    let a = performance.now();
    let matches = search.search(state.search);
    let b = performance.now();
    state.matches = markRaw(matches);
    console.log(b - a);
});

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
            search.toggle(restore ? 2 : 0);
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
