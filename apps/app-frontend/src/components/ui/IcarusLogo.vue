<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import { useAccent } from '@/composables/use-accent.ts'
import { ACCENT_COLORS, recolorPixels } from '@/utils/accent-colors.mjs'
import icarusLogo from '@/assets/icarus-logo.png'
const accent = useAccent()
const recolored = ref(icarusLogo)
const image = new Image()
image.src = icarusLogo
const cache = new Map<string, string>()
const color = computed(() => ACCENT_COLORS[accent.active].icon)
function updateLogo() {
	if (!image.complete || !image.naturalWidth) return
	let result = cache.get(color.value)
	if (!result) {
		const canvas = document.createElement('canvas')
		canvas.width = canvas.height = 256
		const context = canvas.getContext('2d')
		if (!context) return
		context.drawImage(image, 0, 0, 256, 256)
		const pixels = context.getImageData(0, 0, 256, 256)
		recolorPixels(pixels.data, color.value)
		context.putImageData(pixels, 0, 0)
		result = canvas.toDataURL()
		cache.set(color.value, result)
	}
	recolored.value = result
}
image.onload = updateLogo
watch(color, updateLogo, { immediate: true })
</script>

<template>
	<span class="icarus-logo" aria-label="Icarus">
		<img :src="recolored" alt="" class="icarus-logo-icon" />
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
