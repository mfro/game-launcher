import { markRaw, reactive, watchEffect } from 'vue';

export const state = reactive({
    search: '',
    matches: [],
    visible: false,
    instance: null,
});
window.state = state;

search.hook(() => {
    search.toggle(1);
    state.visible = true;
});

watchEffect(() => {
    let matches = search.search(state.search);
    state.matches = markRaw(matches);
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
        }, 250);
    }
}
