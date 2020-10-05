<template>
  <div class="root">
    <div class="window" :style="windowStyle">
      <div class="background" :style="menuStyle" />

      <div class="search-bar">
        <input
          ref="input"
          type="text"
          spellcheck="false"
          :value="inputDisplay"
          @input="onInput"
          v-on:keydown.enter="submit()"
          :class="{ overlay: selected != null }"
        />
      </div>

      <div class="results" :style="menuStyle">
        <result
          v-for="result in state.matches"
          :key="result.key"
          :result="result"
          :selected="result == selected"
          :search="state.search"
        />
      </div>
    </div>
  </div>
</template>

<script>
import { watchEffect } from 'vue';

import Result from './Result.vue';
import { hide, state, entry_match } from './app';

const MENU_SIZE = 7;

export default {
  name: 'app',

  components: {
    Result,
  },

  data() {
    return {
      state,
      index: 0,
      animate: false,
    };
  },

  computed: {
    selected() {
      if (state.matches.length == 0)
        return null;
      return state.matches[this.selectedIndex];
    },

    selectedIndex() {
      let limit = Math.min(state.matches.length, MENU_SIZE);
      if (limit == 0)
        return 0;
      let index = this.index % limit;
      if (index < 0)
        index += limit;
      return index;
    },

    windowStyle() {
      let style = {
        'opacity': state.visible ? 1 : 0,
      };

      // if (this.match) {
      //   style = {
      //     ...style,
      //     'color': this.match.foreground,
      //     'background-color': this.match.background,
      //   }
      // }

      return style;
    },

    menuStyle() {
      return {
        top: `${this.selectedIndex * -54}px`,
        transition: this.animate ? 'all 200ms ease-in-out' : '',
      };
    },

    inputDisplay() {
      if (this.selected)
        return this.selected.target.display_name.slice(0, state.search.length);
      return state.search;
    },
  },

  created() {
    state.instance = this;

    watchEffect(() => {
      this.index = Math.max(0, Math.min(state.matches.length - 1, this.index));
    });
  },

  methods: {
    reset() {
      state.search = '';
      this.index = 0;
    },

    submit() {
      this.selected.target.launch();
      hide(false);
    },

    onInput(e) {
      let str = e.target.value;
      let start = Math.min(str.length, this.inputDisplay.length);
      for (let i = 0; i < start; ++i) {
        if (str[i] != this.inputDisplay[i]) {
          start = i;
          break;
        }
      }

      let end1 = str.length;
      let end2 = this.inputDisplay.length;
      for (let i1 = start; i1 < str.length; ++i1) {
        for (let i2 = start; i2 < this.inputDisplay.length; ++i2) {
          if (str.slice(i1) == this.inputDisplay.slice(i2)) {
            end1 = i1;
            end2 = i2;
            break;
          }
        }
      }

      // console.log(start, end1, end2, this.$refs.input.selectionStart);
      state.search = state.search.slice(0, start) + str.slice(start, end1) + state.search.slice(end2);
      this.animate = false;

      let caret = this.$refs.input.selectionStart;
      setTimeout(() => {
        this.$refs.input.selectionStart = caret;
        this.$refs.input.selectionEnd = caret;
      }, 1);
    },

    select(delta) {
      let index = (this.index + delta) % state.matches.length;
      if (index < 0) index += state.matches.length;
      this.index = index;
      this.animate = true;
    },
  },
};
</script>

<style lang="scss" scoped>
.root {
  width: 100vw;
  height: 100vh;

  padding-top: (54px * 7);
  display: flex;
  align-items: flex-start;
  justify-content: center;
}

.window {
  position: relative;
  width: 640px;
  transition: all 200ms ease-in-out;
}

.background {
  width: 100%;
  height: 100%;
  position: absolute;
  border-radius: 5px;
  background-color: #bdbdbd;
  box-shadow: 0 0 10px -5px currentColor;
}

.results {
  display: flex;
  flex-direction: column;
  position: relative;
}

.search-bar {
  width: 100%;

  top: 0;
  left: 0;

  position: absolute;
  border-radius: 5px;
  background-color: white;
  box-shadow: 0 0 10px -5px currentColor;

  display: flex;

  input {
    flex: 1 1 0;
    position: relative;
    z-index: 1;

    padding: 12px 16px;
    box-sizing: border-box;
    -webkit-appearance: none;
    border: none;
    outline: none;
    background: none;

    font-size: 24px;
    font-family: Google Sans;
    font-weight: 500;

    &.overlay {
      color: transparent;
      caret-color: #333;
    }
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

head {
  display: none;
}
</style>
