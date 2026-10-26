<script setup lang="ts">
import { Info } from '@lucide/vue'
import { APP_BRAND } from '../../shared/brand'
</script>

<template>
  <section class="about-page" aria-labelledby="about-title">
    <div class="about-content">
      <header class="about-heading">
        <Info :size="18" />
        <div>
          <h1 id="about-title">关于 {{ APP_BRAND.name }}</h1>
          <p>{{ APP_BRAND.chineseName }} · {{ APP_BRAND.tagline }}</p>
        </div>
      </header>

      <p class="about-description">{{ APP_BRAND.description }}</p>

      <dl class="about-meta">
        <div>
          <dt>版本</dt>
          <dd>{{ APP_BRAND.version }}</dd>
        </div>
        <div>
          <dt>作者</dt>
          <dd>{{ APP_BRAND.author }}</dd>
        </div>
      </dl>
    </div>
  </section>
</template>

<style scoped>
.about-page {
  min-width: 0;
  min-height: 0;
  padding: 28px 20px;
  overflow: auto;
}

.about-content {
  width: min(720px, 100%);
  padding-bottom: 22px;
  border-bottom: 1px solid var(--border);
}

.about-heading {
  display: flex;
  align-items: start;
  gap: 10px;
  color: var(--accent-strong);
}

.about-heading h1 {
  margin: 0;
  color: var(--text);
  font-size: 18px;
}

.about-heading p {
  margin: 5px 0 0;
  color: var(--text-muted);
  font-size: 11px;
}

.about-description {
  max-width: 660px;
  margin: 24px 0 18px;
  color: var(--text-secondary);
  font-size: 12px;
  line-height: 1.75;
}

.about-meta {
  display: flex;
  margin: 0;
  gap: 34px;
}

.about-meta div {
  display: grid;
  grid-template-columns: auto auto;
  gap: 10px;
  font-size: 11px;
}

.about-meta dt {
  color: var(--text-muted);
}

.about-meta dd {
  margin: 0;
  color: var(--text);
  font-family: var(--font-mono);
}

@media (max-width: 720px) {
  .about-page {
    padding: 22px 16px;
  }
}
</style>
