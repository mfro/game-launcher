<template>
  <div class="result">
    <div class="name">
      <span class="hint" v-text="prefix" />
      <span v-text="display" />
      <span class="hint" v-text="suffix" />
    </div>

    <div class="icon" v-if="icon" :style="iconStyle" />
  </div>
</template>

<script>
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
      return {
        'background-image': `url(${encodeURI(this.target.display_icon)})`,
      };
    },
  },
};
</script>

<style lang="scss" scoped>
@import "common.scss";

.result {
  display: flex;

  > .name {
    flex: 1 1 0;
    padding: 12px 16px;
    white-space: pre;

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
    width: 48px;
    height: 48px;
    background-size: contain;
    margin: 3px 16px;

    transition: all 250ms;
  }
}
</style>
