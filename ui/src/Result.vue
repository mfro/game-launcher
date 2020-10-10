<template>
  <div class="result">
    <div class="icon" ref="icon" />

    <div class="name" v-if="hint">
      <span class="hint" v-text="match.key.slice(0, match.start)" />
      <span v-text="match.key.slice(match.start, match.end)" />
      <span class="hint" v-text="match.key.slice(match.end)" />
    </div>
    <div class="name" v-else-if="name">
      <span v-text="match.key" />
    </div>
    <div class="name" v-else />
  </div>
</template>

<script>
import { watchEffect } from 'vue';
export default {
  name: 'result',

  props: {
    match: Object,
    target: Object,
    name: { type: Boolean, default: false },
    icon: { type: Boolean, default: false },
    hint: { type: Boolean, default: false },
  },

  computed: {
    prefix() {
      if (this.hint) return this.match.key.slice(0, this.match.start);
      else return '';
    },

    display() {
      if (this.hint) return this.match.key.slice(this.match.start, this.match.end);
      else if (this.name) return this.match.key;
      else return '';
    },

    suffix() {
      if (this.hint) return this.match.key.slice(this.match.end);
      else return '';
    },

    iconStyle() {
      let style = {};
      if (this.icon) {
        style['background-image'] = `url(${encodeURI(this.target.display_icon)})`;
      }

      return style;
    },
  },

  mounted() {
    watchEffect(() => {
      let box = this.$refs.icon;
      if (box == null) return;
      while (box.firstChild) box.removeChild(box.firstChild);

      if (this.icon && this.target.display_icon) {
        box.appendChild(this.target.display_icon);
      }
    });
  },
};
</script>

<style lang="scss" scoped>
@import "common.scss";

.result {
  display: flex;

  > .name {
    flex: 1 1 0;
    @include text-result;

    > span {
      @include text;

      &.hint {
        color: #888;
      }
    }

    // .animate {
    //   transition: color 250ms;
    // }
  }

  > .icon {
    flex: 0 0 auto;
    width: 64px;
    height: 64px;
    background-size: contain;
    margin: 2px 16px;

    transition: all 250ms;
  }
}
</style>
