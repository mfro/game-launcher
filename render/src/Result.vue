<template>
  <div class="result">
    <div class="name">
      <span v-text="selected ? name.slice(0, input.length) : ''" />
      <span
        :class="{ selected }"
        v-text="selected ? name.slice(input.length) : name"
      />
    </div>

    <div class="icon" :style="iconStyle" />
  </div>
</template>

<script>
import { entry_display } from './app';

export default {
  name: 'result',

  props: {
    result: Object,
    input: String,
    selected: Boolean,
  },

  computed: {
    name() {
      return entry_display(this.result);
    },

    iconStyle() {
      let style = {
        'opacity': 0,
      };

      if (this.result) {
        let path;
        if (this.result.icon)
          path = `app://icons/${this.result.icon}`;
        else if ('names' in this.result)
          path = `app://link/${this.result.path}`;
        else
          path = `app://icons/${this.result.name}.png`;
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
};
</script>

<style lang="scss" scoped>
.result {
  display: flex;

  .name {
    flex: 1 1 0;
    padding: 12px 16px;
    white-space: pre;

    span {
      font-size: 24px;
      font-family: Google Sans;
      font-weight: 500;
      transition: color 250ms;
    }

    .selected {
      color: #888;
    }
  }

  .icon {
    flex: 0 0 auto;
    width: 48px;
    height: 48px;
    background-size: contain;
    margin: 3px 16px;

    transition: all 250ms;
  }
}
</style>
