package com.golammowla.gdm.data.database

import androidx.room.Database
import androidx.room.RoomDatabase
import com.golammowla.gdm.data.database.dao.DownloadDao
import com.golammowla.gdm.data.database.entity.DownloadEntity

@Database(entities = [DownloadEntity::class], version = 1, exportSchema = false)
abstract class GdmDatabase : RoomDatabase() {
    abstract fun downloadDao(): DownloadDao

    companion object {
        const val DATABASE_NAME = "gdm_downloads.db"
    }
}
