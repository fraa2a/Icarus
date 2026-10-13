<script setup lang="ts">
import { ImportIcon, SpinnerIcon } from '@modrinth/assets'
import { Button, Checkbox, commonMessages, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref } from 'vue'

import { toError } from '@/helpers/errors'
import { importModrinth, migrationInstances } from '@/helpers/icarus'
import { instanceKeys } from '@/pages/instance/query-options'

const { formatMessage } = useVIntl()
const messages = defineMessages({
	title: {
		id: 'icarus.migration.title',
		defaultMessage: 'Bring your Modrinth App instances to Icarus',
	},
	body: {
		id: 'icarus.migration.body',
		defaultMessage:
			'Your instances are ready to import. Choose all of them or just your favorites.',
	},
	choose: { id: 'icarus.migration.choose', defaultMessage: 'Choose instances' },
	all: { id: 'icarus.migration.all', defaultMessage: 'Select all' },
	remove: {
		id: 'icarus.migration.remove',
		defaultMessage: 'Remove originals from the Modrinth App after importing',
	},
	removeHint: {
		id: 'icarus.migration.remove-hint',
		defaultMessage:
			'Original instance files will be deleted only after their import succeeds. This cannot be undone.',
	},
	import: {
		id: 'icarus.migration.import',
		defaultMessage: 'Import {count, plural, one {# instance} other {# instances}}',
	},
	busy: { id: 'icarus.migration.busy', defaultMessage: 'Importing instances…' },
	empty: {
		id: 'icarus.migration.empty',
		defaultMessage: 'All available instances have been imported.',
	},
	retry: { id: 'icarus.migration.retry', defaultMessage: 'Check again' },
	failed: {
		id: 'icarus.migration.failed',
		defaultMessage: 'Could not check for Modrinth App instances.',
	},
})
const queryClient = useQueryClient()
const candidatesQuery = useQuery({
	queryKey: ['icarus', 'migration-instances'],
	queryFn: migrationInstances,
	networkMode: 'always',
	retry: false,
})
const candidates = computed(() => candidatesQuery.data.value ?? [])
const modal = ref<InstanceType<typeof NewModal>>()
const selected = ref(new Set<string>())
const removeSource = ref(false)
const busy = ref(false)
const error = ref('')
const selectedIds = computed(() =>
	candidates.value
		.filter((instance) => selected.value.has(instance.id))
		.map((instance) => instance.id),
)
const allSelected = computed(
	() => candidates.value.length > 0 && selectedIds.value.length === candidates.value.length,
)

function toggle(id: string, checked: boolean) {
	const next = new Set(selected.value)
	if (checked) next.add(id)
	else next.delete(id)
	selected.value = next
}

async function show() {
	if (busy.value) return
	error.value = ''
	removeSource.value = false
	selected.value = new Set(candidates.value.map((instance) => instance.id))
	modal.value?.show()
	await candidatesQuery.refetch()
	selected.value = new Set(candidates.value.map((instance) => instance.id))
}

async function importSelected() {
	if (busy.value || selectedIds.value.length === 0) return
	busy.value = true
	error.value = ''
	let imported = false
	try {
		await importModrinth(selectedIds.value, removeSource.value)
		imported = true
	} catch (err) {
		error.value = toError(err).message
	} finally {
		await Promise.all([
			queryClient.invalidateQueries({ queryKey: instanceKeys.all }),
			candidatesQuery.refetch(),
		])
		busy.value = false
	}
	if (imported) modal.value?.hide()
}
</script>

<template>
	<div
		v-if="candidates.length > 0 || candidatesQuery.isError.value"
		class="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-solid border-surface-5 bg-surface-2 p-4"
	>
		<div class="flex min-w-0 flex-col gap-1">
			<span class="font-semibold text-contrast">{{ formatMessage(messages.title) }}</span>
			<span>{{
				formatMessage(candidatesQuery.isError.value ? messages.failed : messages.body)
			}}</span>
		</div>
		<Button
			v-if="candidatesQuery.isError.value"
			:disabled="candidatesQuery.isFetching.value"
			@click="candidatesQuery.refetch()"
			>{{ formatMessage(messages.retry) }}</Button
		>
		<Button v-else type="colored" color="brand" :disabled="busy" @click="show"
			><ImportIcon />{{ formatMessage(messages.choose) }}</Button
		>
	</div>
	<NewModal
		ref="modal"
		:header="formatMessage(messages.title)"
		max-width="560px"
		:disable-close="busy"
	>
		<div class="flex flex-col gap-4" :aria-busy="busy">
			<p class="m-0">{{ formatMessage(messages.body) }}</p>
			<Checkbox
				:model-value="allSelected"
				:indeterminate="selectedIds.length > 0 && !allSelected"
				:disabled="busy || candidatesQuery.isFetching.value"
				:label="formatMessage(messages.all)"
				@update:model-value="
					selected = new Set($event ? candidates.map((instance) => instance.id) : [])
				"
			/>
			<div class="flex max-h-64 flex-col gap-3 overflow-y-auto rounded-xl bg-surface-2 p-3">
				<Checkbox
					v-for="instance in candidates"
					:key="instance.id"
					:model-value="selected.has(instance.id)"
					:disabled="busy || candidatesQuery.isFetching.value"
					:description="instance.name"
					@update:model-value="toggle(instance.id, $event)"
				>
					<span class="flex min-w-0 flex-col"
						><span class="font-semibold text-contrast">{{ instance.name }}</span
						><span class="text-sm text-secondary"
							>{{ instance.loader }} {{ instance.game_version }}</span
						></span
					>
				</Checkbox>
				<span v-if="candidates.length === 0">{{ formatMessage(messages.empty) }}</span>
			</div>
			<Checkbox v-model="removeSource" :disabled="busy" :label="formatMessage(messages.remove)" />
			<p v-if="removeSource" class="m-0 text-sm text-orange">
				{{ formatMessage(messages.removeHint) }}
			</p>
			<p v-if="error" role="alert" class="m-0 text-red">{{ error }}</p>
			<p v-if="candidatesQuery.isError.value" role="alert" class="m-0 text-red">
				{{ formatMessage(messages.failed) }}
			</p>
		</div>
		<template #actions>
			<div class="flex justify-end gap-2">
				<Button type="outlined" :disabled="busy" @click="modal?.hide()">{{
					formatMessage(commonMessages.cancelButton)
				}}</Button>
				<Button
					type="colored"
					color="brand"
					:disabled="
						busy ||
						candidatesQuery.isFetching.value ||
						candidatesQuery.isError.value ||
						selectedIds.length === 0
					"
					@click="importSelected"
					><SpinnerIcon v-if="busy" class="animate-spin" /><ImportIcon v-else />{{
						busy
							? formatMessage(messages.busy)
							: formatMessage(messages.import, { count: selectedIds.length })
					}}</Button
				>
			</div>
		</template>
	</NewModal>
</template>
