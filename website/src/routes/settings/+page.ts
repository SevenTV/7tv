import { redirect } from "@sveltejs/kit";
import { user } from "$/lib/stores";
import { get } from "svelte/store";

export function load() {
	if (!get(user)) {
		redirect(303, "/sign-in?continue=/settings");
	}
}
