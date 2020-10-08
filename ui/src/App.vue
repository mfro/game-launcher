<template>
  <div class="root">
    <canvas ref="canvas" width="840" height="840" />

    <div class="window" :style="windowStyle">
      <div class="background" :style="menuStyle" />

      <div class="search-bar">
        <input
          ref="input"
          type="text"
          spellcheck="false"
          :value="inputDisplay"
          :class="{ sliding, overlay: selected != null }"
          :style="inputStyle"
          @input="onInput"
          v-on:keydown.enter="submit()"
        />

        <div class="inlay-container">
          <div class="inlay" :style="menuStyle">
            <result
              v-for="(match, i) in state.matches"
              :key="i"
              :match="match"
              :target="match.target"
              name
              hint
            />
          </div>
        </div>

        <div class="overlay-container">
          <div class="overlay" :style="menuStyle">
            <result
              v-for="(match, i) in state.matches"
              :key="i"
              :target="match.target"
              icon
            />
          </div>
        </div>
      </div>

      <div class="results" :style="menuStyle">
        <result
          v-for="(match, i) in state.matches"
          :key="i"
          :match="match"
          name
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
      sliding: false,
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
        'transform': `translateY(${this.selectedIndex * -68}px)`,
        'transition': this.sliding ? 'all 200ms ease-out' : '',
      };
    },

    prefix() {
      if (!this.selected)
        return '';

      return this.selected.key.slice(0, this.selected.start);
    },

    inputStyle() {
      if (this.selected && this.$refs.canvas) {
        let context = this.$refs.canvas.getContext('2d');
        context.font = '500 32px Google Sans'

        let fullText = context.measureText(this.selected.key.slice(0, this.selected.end)).width;
        let inputText = context.measureText(this.inputDisplay).width;
        // compute the difference rather than the length of the prefix directly for kerning

        return {
          'text-indent': `${fullText - inputText}px`
        };
      }

      return {};
    },

    inputDisplay() {
      if (this.selected)
        return this.selected.key.slice(this.selected.start, this.selected.end);

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
      let edited = e.target.value;
      let source = this.inputDisplay;

      let start = Math.min(edited.length, source.length);
      for (let i = 0; i < start; ++i) {
        if (edited[i] != source[i]) {
          start = i;
          break;
        }
      }

      let end1 = edited.length;
      let end2 = source.length;
      outer:
      for (let i1 = start; i1 < edited.length; ++i1) {
        for (let i2 = start; i2 < source.length; ++i2) {
          if (edited.slice(i1) == source.slice(i2)) {
            end1 = i1;
            end2 = i2;
            break outer;
          }
        }
      }

      // console.log(edited, source, ':', state.search);
      // console.log(start, end1, end2);
      state.search = state.search.slice(0, start) + edited.slice(start, end1) + state.search.slice(end2);
      this.sliding = false;

      let caret = this.$refs.input.selectionStart;
      setTimeout(() => {
        this.$refs.input.selectionStart = caret;
        this.$refs.input.selectionEnd = caret;
      }, 1);
    },

    select(delta) {
      let index = (this.index + delta) % state.matches.length;
      if (index < 0) index += state.matches.length;
      if (index == this.index) return;
      this.index = index;

      this.sliding = true;
      this.$refs.input.blur();
      clearTimeout(this.timeout);
      this.timeout = setTimeout(() => {
        this.sliding = false;
        this.$refs.input.focus();
      }, 200);
    },
  },
};
</script>

<style lang="scss" scoped>
@import "common.scss";

canvas {
  display: none;
}

.root {
  width: 100vw;
  height: 100vh;

  padding-top: (68px * 6 + 8px) ;
  display: flex;
  align-items: flex-start;
  justify-content: center;

  > canvas {
    @include text;
  }

  > .window {
    position: relative;
    width: 840px;
    transition: all 200ms ease-in-out;

    > .background {
      width: 100%;
      height: 100%;
      position: absolute;
      border-radius: 5px;
      background-color: #bdbdbd;
      box-shadow: 0 0 10px -5px currentColor;
    }

    > .search-bar {
      @include text-result;
      width: 100%;
      z-index: 1;

      top: 0;
      left: 0;
      padding-left: 96px;

      position: absolute;
      border-radius: 5px;
      background-color: white;
      box-shadow: 0 0 10px -5px currentColor;

      display: flex;

      > input {
        @include text;

        flex: 1 1 0;
        position: relative;
        z-index: 1;

        &.overlay {
          color: transparent;
          caret-color: #333;
        }

        &.sliding {
          caret-color: transparent;
        }
      }

      > .inlay-container {
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        overflow: hidden;

        > .inlay {
          position: relative;
          top: 0;
          left: 0;
          width: 100%;
        }
      }

      > .overlay-container {
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;

        > .overlay {
          position: relative;
          top: 0;
          left: 0;
          width: 100%;
        }
      }
    }

    > .results {
      display: flex;
      flex-direction: column;
      position: relative;
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
