<script setup lang="ts">
import icarusLogo from '@/assets/icarus-logo.png'
</script>

<template>
	<span class="icarus-logo" aria-label="Icarus">
		<img :src="icarusLogo" alt="" class="icarus-logo-icon" />
		<span>Icarus</span>
	</span>
</template>

<style scoped>
.icarus-logo {
	display: inline-flex;
	align-items: center;
	gap: 0.4em;
	font-size: 1.75rem;
	font-weight: 800;
	line-height: 1;
	letter-spacing: -0.04em;
}

.icarus-logo-icon {
	height: 100%;
	width: auto;
	aspect-ratio: 1;
	object-fit: contain;
}
</style>
