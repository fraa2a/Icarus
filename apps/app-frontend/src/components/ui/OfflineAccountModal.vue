<script setup lang="ts">
import { Button, Input, NewModal } from '@modrinth/ui'
import { computed, ref } from 'vue'

import { create_offline_user } from '@/helpers/auth'

const emit = defineEmits<{
	created: [account: { profile: { id: string; name: string }; offline: boolean }]
}>()
const modal = ref<InstanceType<typeof NewModal>>()
const name = ref('')
const saving = ref(false)
const error = ref('')
const valid = computed(
	() => name.value.length >= 3 && name.value.length <= 16 && !/[^A-Za-z0-9_]/.test(name.value),
)

function show() {
	name.value = ''
	error.value = ''
	modal.value?.show()
}

async function submit() {
	if (!valid.value || saving.value) return
	saving.value = true
	error.value = ''
	try {
		const account = await create_offline_user(name.value)
		emit('created', account)
		modal.value?.hide()
	} catch (cause) {
		error.value = String(cause)
	} finally {
		saving.value = false
	}
}

defineExpose({ show })
</script>

<template>
	<NewModal ref="modal" header="Add offline account" width="500px" max-width="500px">
		<form class="flex flex-col gap-3" @submit.prevent="submit">
			<label class="flex flex-col gap-2">
				<span class="font-semibold text-contrast">Username</span>
				<Input v-model="name" autocomplete="off" wrapper-class="w-full" :disabled="saving" />
			</label>
			<p class="m-0 text-secondary">
				Use 3–16 letters, numbers, or underscores. Offline accounts can play singleplayer and
				servers with online authentication disabled.
			</p>
			<p v-if="error" role="alert" class="m-0 text-red">{{ error }}</p>
			<Button type="colored" color="brand" :disabled="!valid || saving" @click="submit">
				{{ saving ? 'Adding account…' : 'Add offline account' }}
			</Button>
		</form>
	</NewModal>
</template>
