package com.copypaste.android

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

class ClipboardHistoryAdapter(
    private val onDelete: (String) -> Unit
) : ListAdapter<ClipboardItem, ClipboardHistoryAdapter.ViewHolder>(DIFF) {

    inner class ViewHolder(view: View) : RecyclerView.ViewHolder(view) {
        val tvSnippet: TextView = view.findViewById(android.R.id.text1)
        val tvMeta: TextView = view.findViewById(android.R.id.text2)
    }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): ViewHolder {
        val view = LayoutInflater.from(parent.context)
            .inflate(android.R.layout.simple_list_item_2, parent, false)
        return ViewHolder(view)
    }

    override fun onBindViewHolder(holder: ViewHolder, position: Int) {
        val item = getItem(position)
        val ts = SimpleDateFormat("HH:mm:ss", Locale.getDefault()).format(Date(item.wallTimeMs))
        holder.tvSnippet.text = if (item.isSensitive) "⚠ [sensitive]" else item.snippet.take(80)
        holder.tvMeta.text = "${item.contentType} · $ts"
        holder.itemView.setOnLongClickListener {
            onDelete(item.id)
            true
        }
    }

    companion object {
        val DIFF = object : DiffUtil.ItemCallback<ClipboardItem>() {
            override fun areItemsTheSame(a: ClipboardItem, b: ClipboardItem) = a.id == b.id
            override fun areContentsTheSame(a: ClipboardItem, b: ClipboardItem) = a == b
        }
    }
}
