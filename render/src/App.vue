<template>
  <div class="root">
    <div class="input-container" :style="style">
      <div class="pseudo-box">
        <!-- <div class="input-pseudo">
          <span>{{ input }}</span>
          <div class="caret" />
        </div> -->

        <input
          ref="input"
          type="text"
          spellcheck="false"
          v-model="input"
          v-on:keydown.enter="submit()"
        />
      </div>

      <div class="logo" v-if="logoStyle" :style="logoStyle" />
    </div>
  </div>
</template>

<script>
import yaml from 'js-yaml';
import { reactive } from 'vue';

let instance;
const state = reactive({
  apps: [],
  links: [],
  visible: false,
});

function on_hook() {
  config_toggle(true);
  state.visible = true;
}

function on_config(raw, links) {
  let config = yaml.safeLoad(raw);

  if (Array.isArray(config)) {
    for (let entry of config) {
      state.apps.push(entry);
    }
  }

  for (let link of links) {
    state.links.push(link);
  }
}

config_attach(on_config, on_hook);

function hide(callback) {
  if (state.visible) {
    state.visible = false;
    setTimeout(() => {
      config_toggle(false);
      instance.input = '';
      callback && callback()
    }, 300);
  }
}

window.addEventListener('blur', e => hide());

window.addEventListener('keydown', e => {
  if (e.keyCode == 27) hide();
});

function match_string(search, value) {
  let i = value.toLowerCase().indexOf(search);
  if (i == -1) return null;
  return [i, value.length];
}

export default {
  name: 'app',

  data() {
    return {
      input: '',
    };
  },

  computed: {
    match() {
      if (this.input.length < 3)
        return;

      let input = this.input.toLowerCase();

      let matches = [];
      for (let app of state.apps) {
        let match = match_string(input, app.name);
        if (match === null) continue;
        matches.push([match, app]);
      }

      for (let link of state.links) {
        for (let name of link.names) {
          let match = match_string(input, name);
          if (match === null) continue;
          matches.push([match, link]);
          break;
        }
      }

      matches.sort((a, b) => {
        for (let i = 0; i < a[0].length; ++i)
          if (a[0][i] != b[0][i])
            return a[0][i] - b[0][i];
        return 0;
      });

      return matches[0] && matches[0][1];
    },

    style() {
      let style = {
        'opacity': state.visible ? 1 : 0,
      };

      if (this.match) {
        style = {
          ...style,
          'color': this.match.foreground,
          'background-color': this.match.background,
        }
      }

      return style;
    },

    logoStyle() {
      let style = {
        'opacity': 0,
      };

      if (this.match) {
        let path;
        if (this.match.icon)
          path = `app://icons/${this.match.icon}`;
        else if ('names' in this.match)
          path = `app://link/${this.match.path}`;
        else
          path = `app://icons/${this.match.name}.png`;
        path = encodeURI(path)

        style = {
          ...style,
          'opacity': 1,
          'background-image': `url(${path})`,
        };
      }

      return style;
    },
  },

  created() {
    instance = this;
    window.addEventListener('focus', e => {
      this.$refs.input.focus();
    });
  },

  methods: {
    submit() {
      let args;
      if ('names' in this.match)
        args = [this.match.path];
      else
        args = [[...this.match.target]];
      config_launch(...args);

      hide(() => {
        // config_launch(...args);
      });
    },
  },
};
</script>

<style lang="scss" scoped>
.root {
  width: 100vw;
  height: 100vh;

  display: flex;
  align-items: center;
  justify-content: center;
}

.input-container {
  color: #333;

  width: 480px;
  border-radius: 5px;
  background-color: white;
  overflow: hidden;

  display: flex;

  box-shadow: 0 0 10px -5px currentColor;

  transition: all 250ms;

  .pseudo-box {
    flex: 1 1 0;
    width: 100%;
    padding: 12px 16px 12px 0;
    box-sizing: border-box;

    font-size: 24px;
    font-family: Google Sans;
    font-weight: 500;
  }

  // .input-pseudo {
  //   position: absolute;
  //   text-indent: 16px;
  //   white-space: pre;
  //   display: flex;
  //   align-items: center;

  //   .caret {
  //     width: 0.8ch;
  //     height: 2px;
  //     background-color: currentColor;
  //     opacity: 0.8;
  //     margin: 6px 0;
  //     align-self: flex-end;
  //   }
  //   // &::after {
  //   //   content: '';
  //   //   width: 2px;
  //   //   height: 1em;
  //   //   background-color: red;
  //   // }
  // }

  input {
    width: 100%;
    padding: 0;

    -webkit-appearance: none;
    border: none;
    outline: none;
    background: none;

    // color: inherit;
    // font-size: inherit;
    // font-family: inherit;
    // font-weight: inherit;

    // opacity: 0;

    // caret-color: transparent;
    text-indent: 16px;
  }

  .logo {
    flex: 0 0 auto;
    width: 48px;
    height: 48px;
    background-size: contain;
    margin: 3px 16px;

    transition: all 250ms;
  }
}
</style>

<style lang="scss">
body,
html {
  width: 100vw;
  height: 100vh;
  margin: 0;
  overflow: hidden;
}
</style>
