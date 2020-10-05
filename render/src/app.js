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

const asset_urls = [];
watchEffect(() => {
    let matches = search.search(state.search);

    for (let match of matches) {
        let url = match.target.display_icon;

        if (typeof url == 'number') {
            if (url in asset_urls) {
                match.target.display_icon = asset_urls[url];
            } else {
                let asset = search.assets[url];
                let blob = new Blob([Uint8Array.from(asset.data)], { type: asset.type });
                match.target.display_icon = asset_urls[url] = URL.createObjectURL(blob);
            }
        }
    }

    state.matches = markRaw(matches);
});

// window.addEventListener('blur', e => hide(true));

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
