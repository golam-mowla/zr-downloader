package com.golammowla.gdm.di

import android.content.Context
import androidx.room.Room
import com.golammowla.gdm.data.database.GdmDatabase
import com.golammowla.gdm.data.database.dao.DownloadDao
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object DatabaseModule {

    @Provides
    @Singleton
    fun provideDatabase(@ApplicationContext context: Context): GdmDatabase {
        return Room.databaseBuilder(
            context,
            GdmDatabase::class.java,
            GdmDatabase.DATABASE_NAME
        )
        .fallbackToDestructiveMigration()
        .build()
    }

    @Provides
    fun provideDownloadDao(database: GdmDatabase): DownloadDao {
        return database.downloadDao()
    }
}
