<script>
  import Field from './Field.svelte';
  import Segmented from './Segmented.svelte';
  let { style = $bindable() } = $props();
</script>

<Field label="Tone">
  <Segmented size="sm" value={style.tone} onchange={(v) => (style.tone = v)}
    options={[{ id: 'terse', label: 'Terse', icon: 'bolt' }, { id: 'neutral', label: 'Neutral', icon: 'minus' }, { id: 'friendly', label: 'Friendly', icon: 'smile' }, { id: 'formal', label: 'Formal', icon: 'briefcase' }]} />
</Field>
<Field label="Voice" info="Imperative: Add / Fix. Past: Added / Fixed. We: We add.">
  <Segmented size="sm" value={style.voice} onchange={(v) => (style.voice = v)}
    options={[{ id: 'imperative', label: 'Add', icon: 'pen' }, { id: 'past', label: 'Added', icon: 'history' }, { id: 'we', label: 'We add', icon: 'users' }]} />
</Field>
<Field label="Detail" info="Low: what and why. Medium: key decisions. High: functions, files and trade-offs.">
  <Segmented size="sm" value={style.detail} onchange={(v) => (style.detail = v)}
    options={[{ id: 'low', label: 'Low', icon: 'minimize' }, { id: 'medium', label: 'Medium', icon: 'layout' }, { id: 'high', label: 'High', icon: 'maximize' }]} />
</Field>
<Field label="Also tell agents" stacked>
  <textarea rows="2" bind:value={style.notes} placeholder="Write like I talk: plain, direct, no fluff."></textarea>
</Field>
