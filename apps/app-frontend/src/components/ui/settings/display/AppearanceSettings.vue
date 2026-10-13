<script setup lang="ts">
import {
	AppearanceSettingsLayout,
	defineMessages,
	Toggle,
	useVIntl,
	injectAuth,
	injectUserPreferences,
	provideAppearanceSettings,
	useSavable,
} from '@modrinth/ui'
import { platform } from '@tauri-apps/plugin-os'
import { computed, inject, onBeforeUnmount, onMounted, watch } from 'vue'

import { type AccentPreset, useAccent } from '@/composables/use-accent.ts'
import { ACCENT_COLORS } from '@/utils/accent-colors.mjs'
import { useAppSettings } from '@/composables/use-app-settings.ts'
import { type ColorTheme, isDarkTheme, useTheme } from '@/composables/use-theme.ts'
import { type AppSettings, get, set } from '@/helpers/settings.ts'
import { appSettingsModalContextKey } from '@/providers/app-settings-modal'

const theme = useTheme()
const accent = useAccent()
const { formatMessage } = useVIntl()
const accentMessages = defineMessages({
	title: { id: 'app.appearance.accent.title', defaultMessage: 'Accent color' },
	description: { id: 'app.appearance.accent.description', defaultMessage: 'Personalize buttons, highlights, the splash screen, and the Icarus icon on this device.' },
	tint: { id: 'app.appearance.accent.tint', defaultMessage: 'Tint backgrounds with accent color' },
	purple: { id: 'app.appearance.accent.purple', defaultMessage: 'Purple' },
	green: { id: 'app.appearance.accent.green', defaultMessage: 'Green' },
	blue: { id: 'app.appearance.accent.blue', defaultMessage: 'Blue' },
	orange: { id: 'app.appearance.accent.orange', defaultMessage: 'Orange' },
	rose: { id: 'app.appearance.accent.rose', defaultMessage: 'Rose' },
})
const accentOptions: AccentPreset[] = ['purple', 'green', 'blue', 'orange', 'rose']
const appSettings = useAppSettings()
const auth = injectAuth()
const { updatePreferences } = injectUserPreferences()
const settingsModal = inject(appSettingsModalContextKey, null)
const os = platform()

type AppearanceSettingsState = {
	theme: ColorTheme
	accent: AccentPreset
	backgroundTint: boolean
	syncAcrossDevices: boolean
	advancedRendering: boolean
	nativeDecorations: boolean
}

function getAppearanceSettingsState(): AppearanceSettingsState {
	return {
		theme: theme.preferred,
		accent: accent.preferred,
		backgroundTint: accent.backgroundTint,
		syncAcrossDevices: theme.syncAcrossDevices,
		advancedRendering: theme.advancedRendering,
		nativeDecorations: appSettings.nativeDecorations,
	}
}

const { saved, current, changes, saving, hasChanges, reset, save } = useSavable(
	getAppearanceSettingsState,
	async (appearanceChanges) => {
		const value = current.value
		if (
			value.syncAcrossDevices &&
			auth.user.value &&
			(appearanceChanges.theme !== undefined || appearanceChanges.syncAcrossDevices !== undefined)
		) {
			await updatePreferences({
				appearance: value.theme === 'system' ? { auto: true } : { auto: false, theme: value.theme },
			})
		}

		const nextSettings: AppSettings = {
			...(await get()),
			theme: value.theme,
			sync_theme_across_devices: value.syncAcrossDevices,
			advanced_rendering: value.advancedRendering,
			native_decorations: value.nativeDecorations,
		}

		await set(nextSettings)
		if (isDarkTheme(value.theme)) {
			theme.preferredDark = value.theme
		}
		theme.preferred = value.theme
		theme.syncAcrossDevices = value.syncAcrossDevices
		theme.advancedRendering = value.advancedRendering
		appSettings.nativeDecorations = value.nativeDecorations
		accent.preferred = value.accent
		accent.backgroundTint = value.backgroundTint
	},
)

const themeOptions = computed(() =>
	theme.options.filter(
		(option) => option !== 'retro' || appSettings.devMode || current.value.theme === 'retro',
	),
)

const preferredDarkTheme = computed(() =>
	isDarkTheme(current.value.theme) ? current.value.theme : theme.preferredDark,
)

function setTheme(value: ColorTheme): void {
	current.value.theme = value
}

function setSyncAcrossDevices(enabled: boolean): void {
	current.value.syncAcrossDevices = enabled
}

function setAdvancedRendering(enabled: boolean): void {
	current.value.advancedRendering = enabled
}

function setNativeDecorations(enabled: boolean): void {
	current.value.nativeDecorations = enabled
}

watch(
	[() => current.value.theme, () => saved.value.theme],
	([selectedTheme, savedTheme]) => {
		theme.preview = selectedTheme === savedTheme ? null : selectedTheme
	},
	{ immediate: true },
)

watch(
	[() => current.value.accent, () => current.value.backgroundTint],
	([preset, tint]) => { accent.preview = { preset, tint } },
	{ immediate: true },
)

async function saveAppearanceSettings(): Promise<void> {
	try {
		await save()
	} catch {
		return
	}
}

onMounted(() => {
	settingsModal?.registerUnsavedChangesController({
		hasChanges: () => hasChanges.value,
		getOriginal: () => saved.value,
		getModified: () => changes.value,
		isSaving: () => saving.value,
		reset,
		save: saveAppearanceSettings,
	})
})

onBeforeUnmount(() => {
	theme.preview = null
	accent.preview = null
	settingsModal?.registerUnsavedChangesController(null)
})

provideAppearanceSettings({
	deferPersistence: true,
	theme: {
		current: computed(() => current.value.theme),
		options: themeOptions,
		system: computed(() => (theme.native === 'light' ? 'light' : preferredDarkTheme.value)),
		preferredDark: preferredDarkTheme,
		set: setTheme,
		syncAcrossDevices: {
			value: computed(() => current.value.syncAcrossDevices),
			set: setSyncAcrossDevices,
		},
		syncDisabled: computed(() => !auth.user.value),
	},
	advancedRendering: {
		value: computed(() => current.value.advancedRendering),
		set: setAdvancedRendering,
	},
	nativeDecorations:
		os !== 'macos'
			? {
					value: computed(() => current.value.nativeDecorations),
					set: setNativeDecorations,
				}
			: undefined,
	updatePreferences,
})
</script>

<template>
	<AppearanceSettingsLayout>
		<template #after-theme>
			<section class="mt-8 border-0 border-t border-solid border-divider pt-6">
				<h2 id="accent-color-label" class="m-0 text-xl font-semibold text-contrast">{{ formatMessage(accentMessages.title) }}</h2>
				<p class="m-0 mt-1 text-secondary">{{ formatMessage(accentMessages.description) }}</p>
				<div class="mt-4 flex flex-wrap gap-2" role="group" aria-labelledby="accent-color-label">
					<button v-for="preset in accentOptions" :key="preset" type="button" class="accent-option" :aria-pressed="current.accent === preset" @click="current.accent = preset">
						<span class="accent-swatch" :style="{ background: ACCENT_COLORS[preset][theme.active === 'light' ? 'light' : 'dark'] }" aria-hidden="true" />
						{{ formatMessage(accentMessages[preset]) }}
					</button>
				</div>
				<div class="mt-6 flex items-center justify-between gap-4">
					<label for="accent-background-tint" class="font-semibold text-contrast">{{ formatMessage(accentMessages.tint) }}</label>
					<Toggle id="accent-background-tint" v-model="current.backgroundTint" />
				</div>
			</section>
		</template>
	</AppearanceSettingsLayout>
</template>

<style scoped>
.accent-option {
	display: inline-flex;
	align-items: center;
	gap: 0.5rem;
	padding: 0.65rem 1rem;
	border: 2px solid transparent;
	border-radius: var(--radius-md);
	background: var(--color-button-bg);
	color: var(--color-contrast);
	font: inherit;
	font-weight: 600;
	cursor: pointer;
}
.accent-option[aria-pressed='true'] { border-color: var(--color-brand); background: var(--color-brand-highlight); }
.accent-option:focus-visible { outline: 2px solid var(--color-brand); outline-offset: 3px; }
.accent-swatch { width: 1rem; height: 1rem; border-radius: 50%; }
</style>
