<template>
  <div class="result" :class="{ launch }">
    <div class="icon" ref="icon" />

    <div class="content">
      <div class="name" v-if="hint">
        <span class="hint" v-text="match.key.slice(0, match.start)" />
        <span v-text="match.key.slice(match.start, match.end)" />
        <span class="hint" v-text="match.key.slice(match.end)" />
      </div>
      <div class="name" v-else-if="name">
        <span v-text="match.key" />
      </div>
      <div class="name" v-else />

      <div class="details" v-if="name">
        <span v-text="target.details"/>
      </div>
    </div>
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
    launch: { type: Boolean, default: false },
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

  &.launch {
    transition: transform 250ms ease-in;
    transform: translateX(1000px);
  }

  > .content {
    height: 80px;
    box-sizing: border-box;

    flex: 1 1 0;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 12px 0;
    margin-right: 8px;
    overflow: hidden;

    > .grow {
      flex: 1 1 0;
    }

    > .name {
      padding: 0;
      white-space: pre;

      > span {
        @include text-title;

        &.hint {
          color: #888;
        }
      }
    }

    > .details {
      display: flex;

      > span {
        overflow: hidden;
        white-space: nowrap;
        text-overflow: ellipsis;
        @include text-details;
      }
    }
  }

  > .icon {
    flex: 0 0 auto;
    width: 64px;
    height: 64px;
    background-size: contain;
    margin: 8px;

    transition: all 250ms;
  }
}
</style>
