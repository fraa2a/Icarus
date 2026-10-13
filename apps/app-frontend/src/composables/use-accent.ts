import { invoke, isTauri } from '@tauri-apps/api/core'
import { computed, reactive, ref, watch } from 'vue'

import { useTheme } from '@/composables/use-theme.ts'
import { ACCENT_COLORS, ACCENT_PRESETS, rgb, validAccent } from '@/utils/accent-colors.mjs'

export type AccentPreset = 'purple' | 'green' | 'blue' | 'orange' | 'rose'
const STORAGE_KEY = 'icarus-appearance-accent'

function loadAppearance(): { preset: AccentPreset; tint: boolean } {
	try {
		const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}')
		return { preset: validAccent(stored.preset) as AccentPreset, tint: stored.tint === true }
	} catch {
		return { preset: 'purple', tint: false }
	}
}

const stored = loadAppearance()
const preferred = ref<AccentPreset>(stored.preset)
const backgroundTint = ref(stored.tint)
const preview = ref<{ preset: AccentPreset; tint: boolean } | null>(null)
const active = computed(() => preview.value?.preset ?? preferred.value)
const tint = computed(() => preview.value?.tint ?? backgroundTint.value)
let initialized = false

export function accentStyle(preset: AccentPreset, light: boolean, tinted = false) {
	const palette = ACCENT_COLORS[preset]
	const color = light ? palette.light : palette.dark
	const channels = rgb(color).join(', ')
	const values: Record<string, string> = {
		'--color-brand': color,
		'--color-brand-highlight': `rgba(${channels}, 0.18)`,
		'--color-brand-shadow': `rgba(${channels}, 0.55)`,
		'--color-accent-contrast': light ? '#ffffff' : '#000000',
		'--color-button-bg-selected': light ? color : `rgba(${channels}, 0.18)`,
		'--color-button-text-selected': light ? '#ffffff' : color,
		'--loading-bar-gradient': `linear-gradient(to right, ${color}, ${palette.icon})`,
		'--splash-tint-top': `rgba(${channels}, ${light ? 0.24 : 0.22})`,
		'--splash-tint-bottom': `rgba(${channels}, ${light ? 0.09 : 0.07})`,
		'--splash-overlay': light ? 'rgba(255, 255, 255, 0.3)' : 'rgba(22, 24, 28, 0.64)',
	}
	if (tinted) {
		const base = light ? '#ffffff' : '#16181c'
		values['--color-bg'] = `color-mix(in srgb, ${color} 5%, ${base})`
		values['--color-raised-bg'] = `color-mix(in srgb, ${color} 5%, ${light ? '#f2f4f7' : '#202329'})`
		values['--color-surface-1'] = values['--color-bg']
		values['--color-surface-2'] = values['--color-raised-bg']
	}
	return values
}

export async function syncAccentIcon() {
	if (isTauri()) await invoke('plugin:accent|set_accent_icon', { preset: active.value })
}

export function initializeAccent() {
	if (initialized) return
	initialized = true
	const theme = useTheme()
	let previousKeys: string[] = []
	watch([active, tint, () => theme.active], ([preset, tinted, colorTheme]) => {
		const html = document.documentElement
		for (const key of previousKeys) html.style.removeProperty(key)
		const values = accentStyle(preset, colorTheme === 'light', tinted)
		for (const [key, value] of Object.entries(values)) html.style.setProperty(key, value)
		previousKeys = Object.keys(values)
	}, { immediate: true, flush: 'sync' })
	watch(active, () => { void syncAccentIcon().catch(console.warn) }, { immediate: true })
	watch([preferred, backgroundTint], ([preset, tinted]) => {
		try { localStorage.setItem(STORAGE_KEY, JSON.stringify({ preset, tint: tinted })) } catch { /* Storage can be unavailable. */ }
	})
}

const accent = reactive({ preferred, backgroundTint, preview, active, tint, options: ACCENT_PRESETS })
export function useAccent() { return accent }
